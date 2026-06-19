use super::{ApiLog, Config, HttpRequest, LLMToken, Request, count_bytes_of_llm_tokens};
use async_std::task::sleep;
use crate::{Error, Response, load_json};
use ragit_fs::{WriteMode, exists, join3, write_string};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

impl Request {
    pub fn to_mock_request(&self, config: &Config) -> Result<HttpRequest, Error> {
        let api_key_env_var = self.model.api_key_env_var();
        let api_key = match std::env::var(api_key_env_var) {
            Ok(k) => k.to_string(),
            Err(_) => match config.fallback_api_keys.get(api_key_env_var) {
                Some(k) => k.to_string(),
                None => return Err(Error::ApiKeyNotFound { env_var: String::from(api_key_env_var) }),
            },
        };

        if api_key != "mock-1234" {
            return Err(Error::MockApiKeyError);
        }

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
            response: response.to_string(),
            thinking: None,
            web_search_results: vec![],
            cached_input_tokens: 0,
            input_tokens: self.history.iter().map(
                |turn| count_bytes_of_llm_tokens(&turn.query, 2048) + turn.response.len() as u64
            ).sum::<u64>() + count_bytes_of_llm_tokens(&self.query, 2048),
            output_tokens: (response.len() / 3) as u64,
            log: ApiLog::new(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MockState {
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
                    String::from("The test is complete! Let me check the working directory.\n<read><path>.</path></read>")
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

pub fn reset_mock_state(working_dir: &str) -> Result<(), Error> {
    let state = MockState::new();
    state.store(working_dir)?;
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
            "<ask><to>user</to><question>Test question 12345678</question></ask>",
            None,
        ),
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>logs/summary-files.md</path>\n<content>\nI'm not sure what the user wants me to do. Let me test the tools and see if they're working.\n</content>\n</write>",
            None,
        ),

        // parse test
        MockRequest::new(
            "<read><from>2</from></read>",
            Some("not a valid argument"),
        ),
        MockRequest::new(
            "<read><start>2</start></read>",
            Some("`<path>` in tool `<read>` is missing"),
        ),
        MockRequest::new(
            "<read><start>abc</start><path>logs/</path></read>",
            Some("expected to have type `Integer`"),
        ),
        MockRequest::new(
            "<read><start>-1</start><path>logs/</path></read>",
            Some("is supposed to be in range"),
        ),
        // If there are multiple tool-calls, the parser only takes the first one.
        MockRequest::new(
            "I'll call multiple tools hahaha<read><path>logs/summary-files.md</path></read>I'll read it again haha<read><path>logs/summary-files.md</path></read>",
            Some("test the tools and"),
        ),
        MockRequest::new(
            "
<run>
<env>ADD_ARG_1=8070301090</env>
<env>ADD_ARG_2=309030908</env>
<command>python3 -c \"import os;print(int(os.getenv(\'ADD_ARG_1\')) + int(os.getenv(\'ADD_ARG_2\')))\"</command>
</run>
            ",
            Some("use newline characters"),
        ),
        MockRequest::new(
            "",
            Some("can't find"),
        ),
        // parse test end

        // external path test
        MockRequest::new(
            "<read><path>../ext-file-1</path></read>",
            Some("no such file"),
        ),
        MockRequest::new(
            "<write><path>../ext-file-1</path><mode>create</mode><content>Hello, I am ext-file-1!</content></write>",
            None,
        ),
        MockRequest::new(
            "<read><path>../ext-file-1</path></read>",
            Some("Hello"),
        ),
        MockRequest::new(
            "<remove><path>.././ext-file-1</path></remove>",
            None,
        ),
        MockRequest::new(
            "<read><path>/tmp/ext-file-1</path></read>",
            Some("no such file"),
        ),

        MockRequest::new(
            "<read><path>/tmp/ext-file-2</path></read>",
            Some("no such file"),
        ),
        MockRequest::new(
            "<write><path>/tmp/ext-file-2</path><mode>create</mode><content>Hello, I am ext-file-2!</content></write>",
            None,
        ),
        MockRequest::new(
            "<read><path>/tmp/ext-file-2</path></read>",
            Some("Hello"),
        ),
        MockRequest::new(
            "<remove><path>/tmp/ext-file-2</path></remove>",
            None,
        ),
        MockRequest::new(
            "<read><path>/tmp/ext-file-2</path></read>",
            Some("no such file"),
        ),
        // external path test end

        // agent test
        MockRequest::new(
            "<agent><name>test-agent</name><prompt>Your name is sub-agent-62373095048</prompt></agent>",
            None,
        ),
        // enter sub-agent
        MockRequest::new(
            "<write><path>agent-test.txt</path><mode>create</mode><content>Hello from sub-agent-62373095048</content></write>",
            None,
        ),
        MockRequest::new(
            "<write><path>logs/done</path><mode>create</mode><content>I'll hand-over to the parent agent.</content></write>",
            None,
        ),
        // exit sub-agent
        MockRequest::new(
            "<read><path>agent-test.txt</path></read>",
            Some("62373095048"),
        ),
        // agent test end

        // patch test
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>whatever.md</path>\n<content>
This is line 0.
This is line 1.
This is line 2.
This is line 3.
This is line 4.
This is line 5.
</content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<patch>\n<path>whatever.md</path>\n<diff>
 This is line 1.
-This is line 2.
+This is not line 2.
-This is line 3.
+This is not line 3.
 This is line 4.
</diff>\n</patch>",
            None,
        ),
        MockRequest::new(
            "<read><path>whatever.md</path></read>",
            Some(
"This is line 1.
This is not line 2.
This is not line 3.
This is line 4.",
            ),
        ),
        MockRequest::new(
            "<patch>\n<path>whatever.md</path>\n<diff>
-This is not line 2.
+This is never line 2.
</diff>\n</patch>",
            None,
        ),
        // patch test end

        // some common tool-call-errors test
        MockRequest::new(
            r#"
<write>
<path>new-dir/</path>
<mode>create</mode>
<content></content>
</write>
"#,
            Some("can't create a directory"),
        ),
        MockRequest::new(
            r#"
<write>
<path>whatever.md/invalid.md</path>
<mode>create</mode>
<content></content>
</write>
"#,
            Some("already exists and is not a directory"),
        ),
        MockRequest::new(
            r#"
<read>
<path>new-dir/</path>
</read>
"#,
            Some("no such file"),
        ),
        // some common tool-call-errors test end

        // env var test
        MockRequest::new(
            "<write><path>env_var_test.py</path><mode>create</mode><content>import os; print(os.getenv(\"NEUKGU_TEST_ENV_VAR\", \"not found\"))</content></write>",
            None,
        ),
        MockRequest::new(
            "<run><command>python3 env_var_test.py</command></run>",
            Some("not found"),
        ),
        MockRequest::new(
            "<run><env>NEUKGU_TEST_ENV_VAR=hello-from-neukgu</env><command>python3 env_var_test.py</command></run>",
            Some("hello-from-neukgu"),
        ),
        MockRequest::new(
            "
<run>
<env>
ADD_ARG_1=8070301090
ADD_ARG_2=309030908
</env>
<command>python3 -c \"import os;print(int(os.getenv(\'ADD_ARG_1\')) + int(os.getenv(\'ADD_ARG_2\')))\"</command>
</run>
            ",
            Some("8379331998"),
        ),
        // env var test end
        // cargo test
        MockRequest::new(
            "<run>\n<command>cargo new new_crate</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<run>\n<command>cargo run --manifest-path new_crate/Cargo.toml</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<run>\n<path>new_crate</path>\n<command>cargo run</command>\n</run>",
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
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>logs/summary-cargo.md</path>\n<content>\nI have tested cargo and it's working.\n</content>\n</write>",
            None,
        ),
        // cargo test end

        // git test
        MockRequest::new(
            "<run>\n<command>git init</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>foo/bar/baz</path>\n<content>\nHi! I am foo-bar-baz\n</content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<read>\n<path>foo/bar/baz</path></read>",
            Some("foo-bar-baz"),
        ),
        MockRequest::new(
            "<run>\n<command>git clean -fd foo/bar</command>\n</run>",
            Some("<exit_code>0</exit_code>"),
        ),
        MockRequest::new(
            "<read>\n<path>foo/bar/baz</path></read>",
            Some("no such file"),
        ),
        MockRequest::new(
            "<write><path>d1/d2/x.txt</path><mode>create</mode><content>Hello!!</content></write>",
            None,
        ),
        MockRequest::new(
            "<run><path>d1/d2/</path><command>git init</command></run>",
            None,
        ),
        MockRequest::new(
            "<run><path>d1/d2/</path><command>git status</command><stdout>../stat.txt</stdout></run>",
            None,
        ),
        MockRequest::new(
            "<read><path>d1/stat.txt</path></read>",
            Some("branch"),
        ),
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>logs/summary-git.md</path>\n<content>\nI have tested git and it's working.\n</content>\n</write>",
            None,
        ),
        // git test end

        // symlink test
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>link.py</path>\n<content>
import os
os.symlink(\"Cargo.toml\", \"symlink-wrong\")
os.symlink(\"new_crate/Cargo.toml\", \"symlink-correct\")
            </content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>python3 link.py</command>\n</run>",
            None,
        ),
        MockRequest::new(
            "<read>\n<path>symlink-wrong</path>\n</read>",
            Some("Cargo.toml"),
        ),
        MockRequest::new(
            "<read>\n<path>symlink-correct</path>\n</read>",
            Some("new_crate/Cargo.toml"),
        ),
        MockRequest::new(
            "<run>\n<command>git add symlink-correct</command>\n</run>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>git commit -m _</command>\n</run>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>git ls-tree HEAD</command>\n</run>",
            Some("120000"),  // a symlink's permission
        ),
        // symlink test end

        // browser test
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>hello.html</path>\n<content>\n<h1>Hello, World!</h1><ul><li>Hello</li><li>World</li></ul>\n</content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<chrome><input>hello.html</input><output>hello.png</output><script>31627 * 31627</script></chrome>",
            Some("1000267129"),
        ),
        MockRequest::new(
            "<chrome><input>hello.html</input><output>hello-2.png</output></chrome>",
            None,
        ),
        MockRequest::new(
            "<chrome><input>https://youtube.com</input><output>youtube.png</output></chrome>",
            None,
        ),
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>logs/summary-browser.md</path>\n<content>\nI have tested the <chrome> tool and it's working.\n</content>\n</write>",
            None,
        ),
        // browser test end

        // svg test (it's not automated, you have to check it manually... I want to see if it can render korean text)
        MockRequest::new(
            r##"<write>
<mode>create</mode>
<path>korean.svg</path>
<content>
<svg xmlns="http://www.w3.org/2000/svg" width="480" height="480" viewBox="0 0 480 480">
  <rect width="480" height="480" fill="#40b040"/>
  <text x="240" y="240"
        text-anchor="middle"
        dominant-baseline="middle"
        font-size="32"
        fill="white">
    Hi, my name is 배현솔
  </text>
</svg>

</content>
</write>"##,
            None,
        ),
        MockRequest::new(
            "<chrome><input>korean.svg</input><output>korean.png</output></chrome>",
            None,
        ),
        // svg test end

        MockRequest::new(
            "<run>\n<command>python3 -c \"print(3162277660168379331998 * 3162277660168379331998)\"</command>\n</run>",
            Some("9999999999999999999994348728804092706672004"),
        ),

        // If you want to test interruption, do it here!
        MockRequest::new(
            "<run>\n<command>python3 -c \"import time; time.sleep(10); print(3162277660168379331998 * 3162277660168379331998)\"</command>\n</run>",
            None,
        ),

        // pip/venv test
        // I chose `unicorn` because I don't think it'd ever be on my machine, system-wide.
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
            "<write>\n<mode>create</mode>\n<path>logs/summary-python.md</path>\n<content>\nI have tested python and it's working.\n</content>\n</write>",
            None,
        ),
        // pip/venv test end

        // PATH test
        // Even if `btm` is in the user's PATH, the sandbox cannot detect this. This test makes sense
        // only if `btm` is in the user's PATH.
        MockRequest::new(
            "<write>\n<mode>create</mode>\n<path>path_test.py</path>\n<content>
import subprocess
subprocess.run(\"btm\")
</content>\n</write>",
            None,
        ),
        MockRequest::new(
            "<run>\n<command>python3 path_test.py</command>\n</run>",
            Some("Error"),
        ),
        MockRequest::new(
            "<write>\n<mode>truncate</mode>\n<path>path_test.py</path>\n<content>
import subprocess
subprocess.run([\"cargo\", \"--help\"])
</content>\n</write>",
            None,
        ),
        // The sandbox can detect `cargo`.
        MockRequest::new(
            "<run>\n<command>python3 path_test.py</command>\n</run>",
            Some("build"),
        ),
        // PATH test end

        // This is suppoed to be an error because there's no summary in `logs/done`
        MockRequest::new(
            "Give me a feedback\n<write>\n<mode>create</mode>\n<path>logs/done</path>\n<content></content>\n</write>",
            None,
        ),
        MockRequest::new(
            "Give me a feedback\n<write>\n<mode>create</mode>\n<path>logs/done</path>\n<content>I called tools in the harness, and it's all working!\n\n1. cargo: working\n2. git: working\n3. browser: working\n4. python: working</content>\n</write>",
            None,
        ),
    ]
}
