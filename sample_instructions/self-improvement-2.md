This is an AI application written in Rust. It implements various AI apis (openai, anthropic, ...). Currently, it supports 2 API endpoints: openai and anthropic. But there's a problem. Anthropic's web search feature is too slow. I want you to investigate whether it's my fault or anthropic's fault. If it's my fault, I want you to fix the code so that anthropic's web-search tool works well.

The current implementation is like below:

1. `src/request.rs`: It defines `struct Request`.
  - You have to instantiate `Request` and call `await request.request(...)` send AI apis.
2. `src/request/anthropic.rs` and `src/request/openai.rs` are translation layers for APIs. For example, it converts `Request` to `AnthropicRequest`. `AnthropicRequest` can be directly converted to a json object, and the json object can be used in API calls.

In order to test your implementation, you can use `ai-request` CLI command.

```
# It works.
cargo run -- ai-request --model=gpt --web-search --prompt="Give me a list of nice AI papers/articles published last week."

# It works, but the AI cannot fetch the list.
cargo run -- ai-request --model=gpt --no-web-search --prompt="Give me a list of nice AI papers/articles published last week."

# It doesn't work (or takes very long time).
cargo run -- ai-request --model=sonnet --web-search --prompt="Give me a list of nice AI papers/articles published last week."

# It works, but the AI cannot fetch the list.
cargo run -- ai-request --model=sonnet --no-web-search --prompt="Give me a list of nice AI papers/articles published last week."
```
