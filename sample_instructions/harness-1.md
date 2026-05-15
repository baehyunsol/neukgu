These are AI coding harnesses. They provide tools for AIs to read/write/execute code.

- https://github.com/openai/codex
- https://github.com/anomalyco/opencode
- https://github.com/Kuberwastaken/claurst

I want you to inspect how these tools implement "edit" tool.
In order for AIs to edit an existing file, they have 2 choices.

1. AI generates the entire content of the file, and the harness overwrites the file.
2. AI tells the harness which part of the code to edit and how it should be edited. Then the harness edits the exact part.

1 is very easy to implement but 2 is not. For example, if the AI tells which lines (line numbers) to edit, that'd be very error-prone. If the AI generates exact string of the code to be edited, I think it'd be tricky with newlines. I want you to inspect how the harnesses implement the tool.

I want you to write report at `docs/codex.md`, `docs/opencode.md` and `docs/claurst.md`.
