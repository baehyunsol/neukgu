# Neukgu

Neukgu is an opinionated coding agent. Currently, it only works with Anthropic API (you need an env var `ANTHROPIC_API_KEY`).

It works like this:

1. You create or initialize a working directory with `neukgu new` or `neukgu init`.
2. You write the instructions to `instruction.md`.
3. Run neukgu, and the it will do the rest.

In order to run this, `git`, `python3`, `cargo` and `ripgrep`. Neukgu will try to find these in your PATH.

## 1. Creating a working directory

`neukgu new my-project` will create a new working directory. Like `cargo new`, you have to chdir to `my-project` after running this command.

You have to manually fill `instruction.md` in `my-project/` after creation.

## 2. Initialize a working directory

You can run `neukgu init` in an existing directory to turn it into a neukgu directory.

It only creates `.neukgu/`. You have to create all the other files (`instruction.md`, `src/`, ...) manually.

## 3. Headless mode

Run `neukgu headless` to run neukgu in headless mode. It leaves no traces unless it panics. In order to see how it's going, you have to look at `.neukgu/`.

## 4. TUI

Run `neukgu tui` to run neukgu in tui mode.

## 5. GUI

Run `neukgu gui` to run neukgu in gui mode.
