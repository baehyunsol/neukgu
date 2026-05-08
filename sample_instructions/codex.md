https://github.com/openai/codex -> This is repository of codex, an ai harness.
Codex provides an environment for AIs, where AIs can read/write files and run programs.
It also handles contexts and memorys of sessions. I want you to reverse-engineer the harness.
I want you to inspect these.

1. There must be an orchestrator agent in the harness. It reads the user instruction and decompose it into sub-tasks and distribute works to sub-agents. There must be a system prompt for the orchestrator. Find the system prompt and summary it.
2. Is there a rollback feature? That is, if the AIs do something wrong, the user can discard the AI's work and rollback the working directory to previous state. If there is, please tell me how it's implemented (e.g. using git's specific command)
3. It must do some kinda context engineering. That is, if the session gets too long, the harness will compact the session so that the AIs don't get lost. Tell me how it implements context engineering.
4. There's a desktop app for codex, but I can't find the source code for the desktop in the repository. Is this in the repository or another project? If it's in the repository, please tell me where the source code is.

First, clone the repository. Second, inspect the repository and answer the 4 questions above. Third, write reports at `docs/XXXX.md`. I want you to create a file per question.

The reports' file names and titles should be like this:

1. `docs/orchestrator.md`: title `# How does codex implement orchestrator?`
2. `docs/rollback.md`: title `# How does codex implement rollback?`
3. `docs/context-engineering.md`: title `# How does codex engineer its context?`
4. `docs/desktop.md`: title `# Where can I find source code for codex desktop app?`
