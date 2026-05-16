This is a rust project. It implements various AI apis (anthropic, openai, gemini, ...).

The implementation is at `src/request.rs`, `src/request/*.rs`, `src/model.rs`, `src/response.rs`, `src/response/*.rs`.

To test this, you can use `ai-request` command, like below:

```sh
cargo run -- ai-request --model=gpt-mini --web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs

# I'm asking a question that's difficult to answer without web-search
cargo run -- ai-request --model=gpt-mini --no-web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs

# haiku doesn't support web-search
cargo run -- ai-request --model=sonnet --web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs

cargo run -- ai-request --model=haiku --no-web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs

cargo run -- ai-request --model=gemini-flash --web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs

cargo run -- ai-request --model=gemini-flash --no-web-search --prompt="Give me a list of nice AI papers/articles published last week." --log-dir=api-logs
```

Run each command with `<run>` tool. I also want you to test their thinking abilities with `--think` flag.

I'll give you the api keys:
