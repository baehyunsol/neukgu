use crate::{Context, Error, subprocess};
use ragit_fs::{exists, join, join3};
use regex::Regex;
use serde::{Deserialize, Serialize};

impl Context {
    // For example, if `bin` is `"git"`, it'll just call `git` and rust's `std::process::Command`
    // will find `/usr/bin/git` using PATH. If the user provided `eval` in `bins/`, then it'll
    // execute `bins/eval`.
    pub fn get_bin_path(&self, sandbox_at: &str, bin: &str) -> Result<String, Error> {
        let real_bin = join3(sandbox_at, "bins", bin)?;

        if exists(&real_bin) {
            Ok(real_bin)
        } else {
            Ok(bin.to_string())
        }
    }
}

pub fn load_available_binaries(working_dir: &str) -> Result<Vec<String>, Error> {
    let mut available_binaries = vec![];
    let mut unavailable_binaries = vec![];
    let bin_list: Vec<(&str, &[&str], &str)> = vec![
        ("git", &["version"], r".*git.*\d+\.\d+.+"),
        ("cargo", &["version"], r".*cargo.*\d+\.\d+.+"),
        // ("python3", &[""], ""),
        ("rg", &["--version"], r".*ripgrep.*\d+\.\d+.+"),
    ];

    match try_init_python_venv(working_dir) {
        Ok(_) => {
            available_binaries.push(String::from("python3"));
            available_binaries.push(String::from("pip"));
        },
        Err(e) => {
            eprintln!("{e:?}");
            unavailable_binaries.push(String::from("python3"));
            unavailable_binaries.push(String::from("pip"));
        },
    }

    for (bin, args, checker) in bin_list.iter() {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();

        match subprocess::run(bin.to_string(), &args, &[], ".", 1, "", false) {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let checker = Regex::new(checker).unwrap();

                if checker.is_match(&stdout) {
                    available_binaries.push(bin.to_string());
                }

                else {
                    unavailable_binaries.push(bin.to_string());
                }
            },
            Err(e) => {
                eprintln!("{e:?}");
                unavailable_binaries.push(bin.to_string());
            },
        }
    }

    if unavailable_binaries.is_empty() {
        Ok(available_binaries)
    }

    else {
        Err(Error::UnavailableBinaries(unavailable_binaries))
    }
}

fn try_init_python_venv(working_dir: &str) -> Result<(), Error> {
    let py_venv = join3(working_dir, ".neukgu", "py-venv")?;
    let python3_link = join3(&py_venv, "bin", "python3")?;

    if exists(&python3_link) {
        return Ok(());
    }

    let output = subprocess::run(
        String::from("python3"),
        &[String::from("-m"), String::from("venv"), String::from("py-venv")],
        &[],
        &join(working_dir, ".neukgu")?,
        5,
        working_dir,
        false,
    )?;

    if output.timeout || !exists(&python3_link) || !exists(&join3(&py_venv, "bin", "pip")?) {
        return Err(Error::FailedToInitPythonVenv);
    }

    Ok(())
}

// Python venv doesn't work on some platforms (e.g. python 3.9 on my Mac book).
// So it checks whether python3 & pip are alive in the sandbox.
pub fn check_python_venv(
    env: &[(String, String)],
    sandbox_at: &str,
    working_dir: &str,
) -> Result<(), Error> {
    let pip_result = subprocess::run(
        String::from("pip"),
        &["help"].iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        env,
        sandbox_at,
        3,
        working_dir,
        true,
    )?;

    if !pip_result.stdout.windows(7).any(|w| w == b"install") || !pip_result.stdout.windows(8).any(|w| w == b"download") {
        eprintln!("---- failed to init python venv ----");
        eprintln!("<command>");
        eprintln!("pip help");
        eprintln!("</command>");
        eprintln!("<stdout>");
        eprintln!("{}", String::from_utf8_lossy(&pip_result.stdout));
        eprintln!("</stdout>");
        eprintln!("<stderr>");
        eprintln!("{}", String::from_utf8_lossy(&pip_result.stderr));
        eprintln!("</stderr>");
        return Err(Error::FailedToInitPythonVenv);
    }

    let py_result = subprocess::run(
        String::from("python3"),
        &["-c", "print(3162277660168379 * 3162277660168379)"].iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        env,
        sandbox_at,
        3,
        working_dir,
        true,
    )?;

    if !py_result.stdout.windows(31).any(|w| w == b"9999999999999997900254631487641") {
        eprintln!("---- failed to init python venv ----");
        eprintln!("<command>");
        eprintln!("python -c \"print(3162277660168379 * 3162277660168379)\"");
        eprintln!("</command>");
        eprintln!("<stdout>");
        eprintln!("{}", String::from_utf8_lossy(&py_result.stdout));
        eprintln!("</stdout>");
        eprintln!("<stderr>");
        eprintln!("{}", String::from_utf8_lossy(&py_result.stderr));
        eprintln!("</stderr>");
        return Err(Error::FailedToInitPythonVenv);
    }

    Ok(())
}

// VIBE NOTE: The parsing stuff is written by sonnet (via neukgu)
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ParseCommandError {
    EmptyInput,
    UnclosedQuote,
    TrailingBackslash,
    InvalidBinaryName(String),
}

fn is_valid_binary_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '\\'))
}

pub fn parse_command(input: &str) -> Result<Vec<String>, ParseCommandError> {
    #[derive(Debug)]
    enum State {
        Normal,
        Quoted,
        QuotedEscape,
    }

    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut current_has_quotes = false;
    let mut first_token_had_quotes = false;
    let mut state = State::Normal;

    for ch in input.chars() {
        match state {
            State::Normal => match ch {
                '"' => {
                    in_token = true;
                    current_has_quotes = true;
                    state = State::Quoted;
                }
                '\\' => {
                    // Outside quotes, treat backslash literally (no special handling)
                    in_token = true;
                    current.push(ch);
                }
                ' ' | '\t' | '\n' => {
                    if in_token {
                        if tokens.is_empty() && current_has_quotes {
                            first_token_had_quotes = true;
                        }
                        tokens.push(current.clone());
                        current.clear();
                        in_token = false;
                        current_has_quotes = false;
                    }
                }
                _ => {
                    in_token = true;
                    current.push(ch);
                }
            },
            State::Quoted => match ch {
                '"' => {
                    // End of quoted section; stay in token (don't push yet)
                    state = State::Normal;
                }
                '\\' => {
                    state = State::QuotedEscape;
                }
                _ => {
                    current.push(ch);
                }
            },
            State::QuotedEscape => {
                current.push(ch);
                state = State::Quoted;
            }
        }
    }

    // Check for unclosed states
    match state {
        State::Quoted | State::QuotedEscape => {
            return Err(ParseCommandError::UnclosedQuote);
        }
        State::Normal => {}
    }

    if in_token {
        if tokens.is_empty() && current_has_quotes {
            first_token_had_quotes = true;
        }
        tokens.push(current);
    }

    // Empty input check
    if tokens.is_empty() {
        return Err(ParseCommandError::EmptyInput);
    }

    // Validate binary name (first token)
    if first_token_had_quotes || !is_valid_binary_name(&tokens[0]) {
        return Err(ParseCommandError::InvalidBinaryName(tokens[0].clone()));
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(
            parse_command("cargo run --release"),
            Ok(vec!["cargo".to_string(), "run".to_string(), "--release".to_string()])
        );
    }

    #[test]
    fn test_quoted_standalone() {
        // Quoted string as a standalone token (separated by space)
        assert_eq!(
            parse_command(r#"cargo run -- ai-request --model gpt --prompt "What is 1 + 1?""#),
            Ok(vec![
                "cargo".to_string(),
                "run".to_string(),
                "--".to_string(),
                "ai-request".to_string(),
                "--model".to_string(),
                "gpt".to_string(),
                "--prompt".to_string(),
                "What is 1 + 1?".to_string(),
            ])
        );
    }

    #[test]
    fn test_quoted_attached() {
        // Quoted string attached to a token via `=`
        assert_eq!(
            parse_command(r#"cargo run -- ai-request --model gpt --prompt="What is 1 + 1?""#),
            Ok(vec![
                "cargo".to_string(),
                "run".to_string(),
                "--".to_string(),
                "ai-request".to_string(),
                "--model".to_string(),
                "gpt".to_string(),
                "--prompt=What is 1 + 1?".to_string(),
            ])
        );
    }

    #[test]
    fn test_escaped_quote_in_quoted() {
        // The instruction's example with escaped quotes around the prompt value
        assert_eq!(
            parse_command(r#"cargo run -- ai-request --model gpt --prompt "What is 1 + 1?""#),
            Ok(vec![
                "cargo".to_string(),
                "run".to_string(),
                "--".to_string(),
                "ai-request".to_string(),
                "--model".to_string(),
                "gpt".to_string(),
                "--prompt".to_string(),
                "What is 1 + 1?".to_string(),
            ])
        );
    }

    #[test]
    fn test_multiple_spaces() {
        assert_eq!(
            parse_command("a   b"),
            Ok(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_empty_quoted_string() {
        // Empty quoted string becomes an empty token
        assert_eq!(
            parse_command(r#"cargo run """#),
            Ok(vec!["cargo".to_string(), "run".to_string(), "".to_string()])
        );
    }

    #[test]
    fn test_unclosed_quote() {
        assert_eq!(
            parse_command(r#"cargo run "hello"#),
            Err(ParseCommandError::UnclosedQuote)
        );
    }

    #[test]
    fn test_trailing_backslash_in_quote() {
        // A backslash at the end of a quoted string (nothing follows it before closing quote)
        assert_eq!(
            parse_command("cargo run \"hello\\"),
            Err(ParseCommandError::UnclosedQuote)
        );
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(parse_command(""), Err(ParseCommandError::EmptyInput));
        assert_eq!(parse_command("   "), Err(ParseCommandError::EmptyInput));
    }

    #[test]
    fn test_invalid_binary_name_quoted() {
        // Binary name was formed from a quoted string
        assert_eq!(
            parse_command(r#""cargo" run"#),
            Err(ParseCommandError::InvalidBinaryName("cargo".to_string()))
        );
    }

    #[test]
    fn test_invalid_binary_name_with_special_chars() {
        // Binary name contains characters not in the allowed set
        assert_eq!(
            parse_command("car!go run"),
            Err(ParseCommandError::InvalidBinaryName("car!go".to_string()))
        );
    }

    #[test]
    fn test_valid_binary_paths() {
        // Path-like binary names should be valid
        assert_eq!(
            parse_command("./my_script --flag"),
            Ok(vec!["./my_script".to_string(), "--flag".to_string()])
        );
        assert_eq!(
            parse_command("/usr/bin/python3 script.py"),
            Ok(vec!["/usr/bin/python3".to_string(), "script.py".to_string()])
        );
    }

    #[test]
    fn test_escaped_quote_inside_quotes() {
        // \"...\" inside a quoted string
        assert_eq!(
            parse_command(r#"cargo run --prompt "say \"hello\"""#),
            Ok(vec![
                "cargo".to_string(),
                "run".to_string(),
                "--prompt".to_string(),
                r#"say "hello""#.to_string(),
            ])
        );
    }
}
