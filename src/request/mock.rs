use super::{HttpRequest, LLMToken, Request, count_bytes_of_llm_tokens};
use async_std::task::sleep;
use crate::{Error, Response, load_json};
use ragit_fs::{WriteMode, exists, join3, write_string};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

impl Request {
    pub fn to_mock_request(&self) -> Result<HttpRequest, Error> {
        Ok(HttpRequest {
            // It doesn't send anything to this url.
            // `url` is set because otherwise `request::Client` will refuse to construct a `RequestBuilder`.
            url: String::from("http://127.0.0.1:8000"),

            headers: HashMap::new(),
            body: serde_json::to_value(self)?,
        })
    }

    pub async fn send_mock_request(&self, working_dir: &str) -> Result<Response, Error> {
        sleep(Duration::from_millis(3000 + rand::random::<u64>() % 4096)).await;
        let mut state = MockState::load(working_dir)?;
        state.check_prev_turn_output(&self.query)?;
        let response = state.get_next_turn();
        state.store(working_dir)?;

        Ok(Response {
            response,
            thinking: None,
            web_search_results: vec![],
            cached_input_tokens: 0,
            input_tokens: self.history.iter().map(
                |turn| count_bytes_of_llm_tokens(&turn.query, 2048) + turn.response.len() as u64
            ).sum::<u64>() + count_bytes_of_llm_tokens(&self.query, 2048),
            output_tokens: 0,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MockState {
    turns: Vec<Option<MockRequest>>,

    // in the next turn, it'll `self.turns.push(all_turns[self.seq])`
    seq: usize,
}

impl MockState {
    pub fn new() -> MockState {
        MockState { turns: vec![], seq: 0 }
    }

    pub fn load(working_dir: &str) -> Result<MockState, Error> {
        let mock_path = join3(working_dir, ".neukgu", "mock.json")?;

        if exists(&mock_path) {
            load_json(&mock_path)
        }

        else {
            Ok(MockState::new())
        }
    }

    pub fn store(&self, working_dir: &str) -> Result<(), Error> {
        Ok(write_string(
            &join3(working_dir, ".neukgu", "mock.json")?,
            &serde_json::to_string_pretty(self)?,
            WriteMode::Atomic,
        )?)
    }

    pub fn check_prev_turn_output(&self, output: &[LLMToken]) -> Result<(), Error> {
        match self.turns.last() {
            Some(Some(MockRequest { expect: Some(expect), .. })) => {
                for token in output.iter() {
                    if let LLMToken::String(s) = token && s.contains(expect) {
                        return Ok(());
                    }
                }

                Err(Error::MockApiExpectationFailure { expect: expect.to_string() })
            },
            _ => Ok(()),
        }
    }

    pub fn get_next_turn(&mut self) -> String {
        if rand::random::<u32>() % 4 == 0 {
            self.turns.push(None);
            String::from("The mock AI occasionally outputs random, non-sense outputs.")
        }

        else {
            let all_turns = mock_requests();

            match all_turns.get(self.seq) {
                Some(r) => {
                    self.turns.push(Some(r.clone()));
                    self.seq += 1;
                    r.request.to_string()
                },
                None => {
                    self.turns.push(None);
                    String::from("The test is complete! Let me check if there's a new instruction.\n<read><path>neukgu-instruction.md</path></read>")
                },
            }
        }
    }
}

pub fn revert_mock_state(working_dir: &str) -> Result<(), Error> {
    let mock_path = join3(working_dir, ".neukgu", "mock.json")?;

    if exists(&mock_path) {
        let mut mock_state: MockState = load_json(&mock_path)?;

        if let Some(Some(_)) = mock_state.turns.pop() {
            mock_state.seq -= 1;
        }

        mock_state.store(working_dir)?;
    }

    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MockRequest {
    request: String,
    expect: Option<String>,
}

impl MockRequest {
    pub fn new(request: &str, expect: Option<&str>) -> MockRequest {
        MockRequest {
            request: request.to_string(),
            expect: expect.map(|s| s.to_string()),
        }
    }
}

fn mock_requests() -> Vec<MockRequest> {
    vec![
        MockRequest::new(
            "<read>\n<path>neukgu-instruction.md</path>\n</read>",
            None,
        ),
        MockRequest::new(
            "<ask><to>user</to><question>I don't see any instructions... what do you want me to do?</question></ask>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>cargo new new_crate</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<run>\n<command>cargo run --manifest-path new_crate/Cargo.toml</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<read>\n<path>new_crate/Cargo.toml</path>\n</read>",
            Some("new_crate"),
        ),
        MockRequest::new(
            r#"
<write>
<mode>truncate</mode>
<path>new_crate/Cargo.toml</path>
<content>
[package]
This is an invalid Cargo.toml hahaha
version = "0.1.0"
edition = "2024"

[dependencies]
</content>
</write>"#,
            None,
        ),
        MockRequest::new(
            "<read>\n<path>new_crate/Cargo.toml</path>\n</read>",
            Some("hahaha"),
        ),
        MockRequest::new(
            "<run>\n<command>cargo run --manifest-path new_crate/Cargo.toml</command>\n</run>",
            Some("<exit_code>101</exit_code>"),
        ),

        // browser test
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>hello.html</path>\n<content>\n<h1>Hello, World!</h1><ul><li>Hello</li><li>World</li></ul>\n</content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<render><input>hello.html</input><output>hello.png</output><script>31627 * 31627</script></render>",
            Some("1000267129"),
        ),
        MockRequest::new(
            "<render><input>hello.html</input><output>hello-2.png</output></render>",
            None,
        ),
        MockRequest::new(
            "<render><input>https://youtube.com</input><output>youtube.png</output></render>",
            None,
        ),
        // browser test end

        MockRequest::new(
            "<run>\n<command>python3 -c \"print(3162277660168379331998 * 3162277660168379331998)\"</command>\n</run>",
            Some("9999999999999999999994348728804092706672004"),
        ),

        // If you want to test interruption, do it here!
        MockRequest::new(
            "<run>\n<command>python3 -c \"import time; time.sleep(10); print(3162277660168379331998 * 3162277660168379331998)\"</command>\n</run>",
            None,
        ),

        // An arbitrary library to test python/pip.
        // I chose this because I don't think many people have this library pre-installed on their machine.
        MockRequest::new(
            "<run>\n<command>python3 -c \"import unicorn\"</command>\n</run>",
            Some("<exit_code>1</exit_code>"),
        ),
        MockRequest::new(
            "<run>\n<command>pip install unicorn</command>\n</run>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>python3 -c \"import unicorn\"</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),

        MockRequest::new(
            "Give me a feedback\n<write>\n<mode>create</mode>\n<path>logs/done</path>\n<content></content>\n</write>",
            None,
        ),
    ]
}
