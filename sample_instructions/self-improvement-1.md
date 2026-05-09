This is an AI application written in Rust. It implements various AI apis (openai, anthropic, ...). I want you to implement a new cli command for testing AI apis.

The API should look like this:

```sh
cargo run -- ai-request --model=sonnet --prompt="What is 1 + 1?"

# --web-search is disabled by default
cargo run -- ai-request --model=gpt --prompt="Recommend me restaurants in reykjavik" --web-search
```

The current implementation is like below:

1. `src/request.rs`: It defines `struct Request`.
  - You have to instantiate `Request` and call `await request.request(...)` send AI apis.
2. `src/request/anthropic.rs` and `src/request/openai.rs` are translation layers for APIs. For example, it converts `Request` to `AnthropicRequest`. `AnthropicRequest` can be directly converted to a json object, and the json object can be used in API calls.
