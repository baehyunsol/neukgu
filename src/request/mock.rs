use super::{HttpRequest, LLMToken, Request, count_bytes_of_llm_tokens};
use async_std::task::sleep;
use crate::{Error, Response, load_json};
use ragit_fs::{WriteMode, exists, join3, write_string};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

impl Request {
    pub fn to_mock_request(&self) -> Result<HttpRequest, Error> {
        Ok(HttpRequest {
            // It doesn't send anything to this url.
            // `url` is set because otherwise `request::Client` will refuse to construct a `RequestBuilder`.
            url: String::from("http://127.0.0.1:8000"),

            headers: HashMap::new(),
            body: json!({}),
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
            input_tokens: self.history.iter().map(
                |turn| count_bytes_of_llm_tokens(&turn.query, 2048) + turn.response.len() as u64
            ).sum::<u64>() + count_bytes_of_llm_tokens(&self.query, 2048),
            output_tokens: 0,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MockState {
    seq: usize,

    // The mock LLM occasionally outputs non-sense outputs.
    // In such cases, this flag is not set.
    // If this flag is set, it checks if the expectation is met.
    valid: bool,
}

impl MockState {
    pub fn new() -> MockState {
        MockState {
            seq: 0,
            valid: true,
        }
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
        if self.seq > 0 && self.valid {
            let turns = mock_requests();
            let prev_turn = &turns[self.seq - 1];

            if let Some(expect) = &prev_turn.expect {
                for token in output.iter() {
                    if let LLMToken::String(s) = token && s.contains(expect) {
                        return Ok(());
                    }
                }

                Err(Error::MockApiExpectationFailure { expect: expect.to_string() })
            }

            else {
                Ok(())
            }
        }

        else {
            Ok(())
        }
    }

    pub fn get_next_turn(&mut self) -> String {
        if rand::random::<u32>() % 4 == 0 {
            self.valid = false;
            String::from("The mock AI occasionally outputs random, non-sense outputs.")
        }

        else {
            let turns = mock_requests();

            match turns.get(self.seq) {
                Some(r) => {
                    self.seq += 1;
                    self.valid = true;
                    r.request.to_string()
                },
                None => {
                    self.valid = false;
                    String::from("The test is complete!")
                },
            }
        }
    }
}

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
            "<write>\n<mode>truncate</mode>\n<path>new_crate/Cargo.toml</path>\n<content>\nThis is an empty file hahaha\n</content>\n</write>",
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
        MockRequest::new(
            "Give me a feedback\n<write>\n<mode>create</mode>\n<path>logs/done</path>\n<content></content>\n</write>",
            None,
        ),
    ]
}
