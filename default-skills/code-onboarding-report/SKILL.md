---
name: code-onboarding-report
description: This skill describes how to write a code-onboarding-report. Use this skill when the user explicitly asks for a code-onboarding-report.
VIBE-NOTE: This SKILL.md was drafted by me (baehyunsol) and written by Sonnet (via neukgu-chat).
---

# Code-Onboarding-Report

The user is new to a repository and wants to get familiar with it. You have to help the user. You'll be given a git repository. Your job is to write a code-onboarding-report.

Follow this workflow in order:
  1. Clone and do an initial inspection of the repository.
  2. Identify the tech stack.
  3. Ask the user the questions listed below.
  4. Finish inspecting the repository.
  5. Write draft.md.
  6. Review draft.md by yourself.
  7. Write the final report.

The user may instruct where to write the report and what format it should be. If they don't, write the report at `code-onboarding-report.md`.

After you identify the tech stack, ask the user:
  1. Are they a software engineer or not?
  2. How familiar are they with the tech stack? (e.g. if the repository uses Python and Rust, ask if they know Python and Rust)

Use the answers to tailor the report:
  - If the user is not a software engineer, avoid jargon and explain technical terms in plain language.
  - If the user is unfamiliar with a language or framework, add a brief "what this is" explanation when first mentioning it.
  - If the user is already an expert in the stack, skip basic explanations and focus on project-specific details.

The report should include:

1. Tech stack
  - What language it uses. If multiple languages are used, what language is used for what feature.
  - Major frameworks/libraries.
  - What DB, if exists, is used and what it's used for.
  - What package manager, if exists, is used.
  - What infrastructure/cloud, if exists, is used.

2. Entry point
  - List the entry points (e.g. main function) and briefly explain what those functions do.
  - If it's a library, list the top-level public items.
  - If it's a CLI application, list the commands.
  - If it's a backend server, list the endpoints.

3. Directory structure overview
  - This is the most important part. When the user wants to do something, the user will read your report and decide what file/directory to read.
  - Create a codebase map (annotated tree) following these rules:

    Inclusion rules:
    - Always include ALL top-level files and directories.
    - For each directory, go deeper only if the directory is architecturally significant (e.g., core logic, config, API layer).
    - Prioritize: entry points, core modules, config files, API definitions, data models.

    Exclusion rules:
    - Skip dependency folders (node_modules, venv, .cargo, etc.).
    - Skip build artifacts and auto-generated files (dist, __pycache__, etc.).
    - Skip asset files (images, fonts) unless they're architecturally relevant.

    Grouping rule (IMPORTANT):
    - If a directory contains many similar files (e.g., 30 model files, 20 migration files), do NOT list them all.
    - Instead, pick 2-3 representative examples and add a note like "# ... and 27 more model files".

    Length rule:
    - The map should contain between 10 and 50 entries in total.
    - If you find yourself listing more than 50, apply the grouping rule more aggressively.
    - If you find yourself listing fewer than 10, go one level deeper into the most important directories.

    Format:
    - Add a short inline comment (#) to every entry explaining its role.

  Example of good output:
  ```
  src/
  ├── main.py              # Entry point, starts the server
  ├── config.py            # Environment variables and app config
  ├── api/
  │   ├── routes.py        # All API route definitions
  │   └── middleware.py    # Auth and logging middleware
  ├── models/
  │   ├── user.py          # User model (representative example)
  │   ├── post.py          # Post model (representative example)
  │   └── ...              # 12 more model files
  ├── services/
  │   └── email.py         # Email sending logic
  └── tests/               # See section 6 for test strategy
  ```

4. How to run this locally
  - Try to run this by yourself, and explain how to do that.
  - If it's a library, explain how to link/import the library.
  - If it has to be deployed, explain how to deploy this.

5. Version control
  - Commit conventions, branching strategies.
  - Issue tracking conventions, if any (e.g., JIRA ticket references in commits, GitHub issue templates).

6. Test strategies
  - How to run the tests, if they exist.
  - How to add a test case after implementing a new feature.
  - How to add an end-to-end test, if possible.
  - How the CI/CD pipeline, if exists, works.

7. Recommended reading order
  - List 5-10 files the user should read first, in order, with a one-line reason why.

8. Issues
  - Scan for TODO/FIXME/HACK comments and summarize notable ones.
  - If there are a significant number of them, mention that the codebase has notable tech debt.

9. Code conventions
  - Coding style and linting rules (e.g., ESLint, Prettier, Black).
  - Recurring design patterns in the codebase.
  - Error handling approach.

When reviewing draft.md, check:
  - Are there any sections based on assumptions? Flag them.
  - Is any section empty or thin due to lack of information? Note it.
  - Is the directory structure explanation clear to a newcomer?

If you're unsure about something, say so explicitly in the report rather than guessing. Use phrases like "likely", "appears to be", or "couldn't determine".
