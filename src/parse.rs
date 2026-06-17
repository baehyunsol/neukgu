use crate::{AskTo, Config, InterruptId, LLMToken, ToolCall, ToolKind};
use crate::tool::{
    LineDiff,
    ParseCommandError,
    QuestionKind,
    QuestionToUser,
    WebOrFile,
    WriteMode,
    parse_command,
    parse_line_diff,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::{Entry, HashMap};
use std::sync::LazyLock;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParsedSegment {
    pub cot: String,
    pub tool: Option<ToolCall>,

    // original str of the tool call
    pub tool_str: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ParseState {
    String,
    Tool {
        kind: ToolKind,

        // tool xmls are always 2 levels, so it's easy to parse!!
        top_level: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ParseError {
    NoTool,
    InvalidTool(String),
    InvalidArg {
        tool: String,
        arg: String,
        valid_args: Vec<String>,
    },
    MissingArg {
        tool: String,
        arg: String,
    },
    ArgRepeated {
        tool: String,
        arg: String,
    },
    UnterminatedArg {
        tool: String,
        arg: String,
    },
    ArgTypeError {
        tool: String,
        expected_type: ArgType,
        arg_name: String,
        arg: String,
    },
    InvalidIntegerRange {
        tool: String,
        arg_name: String,
        min: Option<i64>,
        max: Option<i64>,
        n: i64,
    },
    InvalidWriteMode(String),
    InvalidPatchPrefix {
        line: String,
        prefix: char,
    },
    InvalidCommand(ParseCommandError),
    NotBash { tried_cd: bool },
    InvalidEnv(String),
    InvalidAskTo(String),
    InvalidImageSize(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ArgType {
    Integer,
}

impl ParseError {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        let s = match self {
            ParseError::NoTool => String::from("
I can't find any XML-syntaxed tool calls in your response.
Please call a tool.
"),
            ParseError::InvalidTool(tool) => format!(
                "`<{tool}>` is not a valid tool. Available tools are {}.",
                config.activated_tools.iter().map(
                    |tool| format!("<{}>", tool.tag_name())
                ).collect::<Vec<_>>().join(", "),
            ),
            ParseError::InvalidArg { tool, arg, valid_args } => format!(
                "`<{arg}>` is not a valid argument for tool `<{tool}>`. Available arguments are {}.",
                valid_args.iter().map(|arg| format!("<{arg}>")).collect::<Vec<_>>().join(", "),
            ),
            ParseError::MissingArg { tool, arg } => format!(
                "Argument `<{arg}>` in tool `<{tool}>` is missing. I can't find `<{arg}>..</{arg}>`.",
            ),
            ParseError::ArgRepeated { tool, arg } => {
                if tool == "run" && arg == "env" {
                    format!(
                        "
Argument `<{arg}>` in tool `<{tool}>` is repeated. If you want to insert multiple env vars, use newline characters.
e.g.
<run>
<env>
KEY1=VALUE1
KEY2=VALUE2
</env>
<command>...</command>
</run>
                        ",
                    )
                }

                else {
                    format!("Argument `<{arg}>` in tool `<{tool}>` is repeated. You can use it only once.")
                }
            },
            ParseError::UnterminatedArg { tool, arg } => format!(
                "Argument `<{arg}>` in tool `<{tool}>` is not terminated properly. I can't find `</{arg}>`.",
            ),
            ParseError::ArgTypeError { tool, expected_type, arg_name, arg } => format!(
                "Argument `<{arg_name}>` in `<{tool}>` is expected to have type `{expected_type:?}`, but it doesn't. I've got `{arg}`.",
            ),
            ParseError::InvalidIntegerRange { tool, arg_name, min, max, n } => format!(
                "Argument `<{arg_name}>` in `<{tool}>` is supposed to be in range {}, but is {n}.",
                match (min, max) {
                    (Some(min), Some(max)) => format!("{min}..={max}"),
                    (Some(min), _) => format!("{min}.."),
                    (_, Some(max)) => format!("..={max}"),
                    (None, None) => unreachable!(),
                },
            ),
            ParseError::InvalidWriteMode(mode) => format!(
                "`{mode}` is not a valid <mode>. Available modes are create, truncate and append.",
            ),
            ParseError::InvalidPatchPrefix { line, prefix } => format!(
                "There's a syntax error in line {line:?}. A line must start with either ' ', '+' or '-', but it starts with {prefix:?}.{}",
                if *prefix == '@' { " If you want to apply multiple patches, you have to call this tool multiple times. Each time with 1 hunk." } else { "" },
            ),
            ParseError::InvalidCommand(ParseCommandError::EmptyInput) => String::from("<command> is empty. Please tell me what command you want to run."),
            ParseError::InvalidCommand(ParseCommandError::UnclosedQuote) => String::from("<command> has an unclosed quote."),
            ParseError::InvalidCommand(ParseCommandError::TrailingBackslash) => String::from("<command> has a trailing backslash."),
            ParseError::InvalidCommand(ParseCommandError::InvalidBinaryName(bin)) => format!("`{bin}` is not a valid name of a binary."),
            ParseError::NotBash { tried_cd } => format!("
Failed to run the command.

You're not using bash. You're directly executing the binary with the arguments.
So you can't use bash features like redirection and pipes.
In order to prevent confusion, the environment rejects your command if there's '&', '>', '<' or '|' character in the cli args.

If the command's stdout and stderr are going to be short, you don't have to redirect it.
Just run the command and see the result.

If the command's stdout or stderr are going to be long, you can use `<stdout>` or `<stderr>` tags. It works like

<run>
<stderr>tests/result-1.txt</stderr>
<command>cargo test</command>
</run>

If you want to do more complex stuff (e.g. end-to-end test), I recommend you write a python script and use python command.

<run>
<command>python3 tests/your_e2e_test.py</command>
</run>{}",
                if *tried_cd {
                    "

If you want to run the binary in another directory, use `<path>` parameter, like this:

<run>
<path>path-to-run-binary/</path>
<command>your-command</command>
</run>"
                } else {
                    ""
                },
            ),
            ParseError::InvalidEnv(line) => format!(
                "{line:?} is an invalid format for env var. It has to look like <env>KEY=VALUE</env>",
            ),
            ParseError::InvalidAskTo(to) => format!(
                "You can't ask to `{to}`. You can ask to either user or web.",
            ),
            ParseError::InvalidImageSize(size) => format!(
                "{size:?} is an invalid format for an image size. It has to be like \"720x720\" or \"1080x1440\"",
            ),
        };

        vec![LLMToken::String(s)]
    }
}

// Most LLMs, especially gpts try to call multiple tools per turn. I tried to prevent that with
// prompts, but I couldn't. Instead, if there are multiple tool calls, the parser only takes the
// first tool and ignores the others.
pub fn parse(input: &[u8], activated_tools: &[ToolKind]) -> Result<ParsedSegment, ParseError> {
    let mut cursor = 0;
    let mut cot = String::new();
    let mut tool = None;
    let mut tool_str = None;
    let mut buffer = vec![];
    let mut state = ParseState::String;

    let mut tool_call_start = 0;
    let mut curr_tag_name = vec![];
    let mut curr_arg_name = vec![];
    let mut args = HashMap::new();

    // If the first found tag is invalid, this var remembers the name.
    // It's later used to generate error messages.
    let mut first_tag_but_wrong_name = None;
    let mut first_tag_but_wrong_arg = None;

    loop {
        match &state {
            ParseState::String => match input.get(cursor) {
                Some(b'<') => {
                    let maybe_tag = check_tag_name(&input[cursor..]);
                    tool_call_start = cursor;

                    match maybe_tag {
                        Some(t) => match ToolKind::from_name(t) {
                            Some(tool_kind) if activated_tools.contains(&tool_kind) => {
                                curr_tag_name = t.to_vec();
                                args = HashMap::new();

                                if !buffer.is_empty() {
                                    cot = String::from_utf8_lossy(&buffer).to_string();
                                    buffer = vec![];
                                }

                                cursor += t.len() + 2;  // 2 for '<' and '>'.
                                state = ParseState::Tool { kind: tool_kind, top_level: true };
                            },
                            _ => {
                                if first_tag_but_wrong_name.is_none() {
                                    first_tag_but_wrong_name = Some(t.to_vec());
                                }

                                buffer.push(b'<');
                                cursor += 1;
                            },
                        },
                        None => {
                            buffer.push(b'<');
                            cursor += 1;
                        },
                    }
                },
                Some(b) => {
                    buffer.push(*b);
                    cursor += 1;
                },
                None => {
                    if !buffer.is_empty() {
                        cot = String::from_utf8_lossy(&buffer).to_string();
                    }

                    return if tool_str.is_none() {
                        match first_tag_but_wrong_name {
                            Some(wrong_tag) => Err(ParseError::InvalidTool(String::from_utf8_lossy(&wrong_tag).to_string())),
                            None => Err(ParseError::NoTool),
                        }
                    } else {
                        Ok(ParsedSegment { cot, tool, tool_str })
                    };
                },
            },
            ParseState::Tool { kind, top_level: true } => match (input.get(cursor), input.get(cursor + 1)) {
                (Some(b'\n' | b' ' | b'\t'), _) => {
                    cursor += 1;
                },
                (Some(b'<'), Some(b'/')) => {
                    let end_tag = format!("</{}>", String::from_utf8_lossy(&curr_tag_name).to_string());
                    let end_tag = end_tag.as_bytes();

                    if input.get(cursor..(cursor + end_tag.len())) == Some(end_tag) {
                        cursor += end_tag.len();
                        tool = Some(ToolCall::parse(*kind, &args)?);
                        tool_str = Some(String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string());
                        return Ok(ParsedSegment { cot, tool, tool_str });
                    }

                    else {
                        todo!()
                    }
                },
                (Some(b'<'), _) => match check_tag_name(&input[cursor..]) {
                    Some(t) => {
                        if kind.check_arg_name(t) {
                            curr_arg_name = t.to_vec();
                            cursor += t.len() + 2;
                            state = ParseState::Tool { kind: *kind, top_level: false };
                        }

                        else {
                            return Err(ParseError::InvalidArg {
                                tool: String::from_utf8_lossy(&curr_tag_name).to_string(),
                                arg: String::from_utf8_lossy(t).to_string(),
                                valid_args: kind.valid_args(),
                            });
                        }
                    },
                    None => {
                        if first_tag_but_wrong_arg.is_none() {
                            first_tag_but_wrong_arg = Some(curr_tag_name.to_vec());
                        }

                        cot = String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string();
                        state = ParseState::String;
                    },
                },
                _ => {
                    if first_tag_but_wrong_arg.is_none() {
                        first_tag_but_wrong_arg = Some(curr_tag_name.to_vec());
                    }

                    cot = String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string();
                    state = ParseState::String;
                },
            },
            ParseState::Tool { kind, top_level: false } => {
                let end_tag = format!("</{}>", String::from_utf8_lossy(&curr_arg_name).to_string());
                let end_tag = end_tag.as_bytes();
                let arg_start = cursor;

                loop {
                    if input.get(cursor..(cursor + end_tag.len())) == Some(end_tag) {
                        match args.entry(curr_arg_name.to_vec()) {
                            Entry::Occupied(_) => {
                                return Err(ParseError::ArgRepeated {
                                    tool: String::from_utf8_lossy(&curr_tag_name).to_string(),
                                    arg: String::from_utf8_lossy(&curr_arg_name).to_string(),
                                });
                            },
                            Entry::Vacant(e) => {
                                e.insert(input[arg_start..cursor].to_vec());
                            },
                        }

                        cursor += end_tag.len();
                        state = ParseState::Tool { kind: *kind, top_level: true };
                        break;
                    }

                    else if cursor > input.len() {
                        return Err(ParseError::UnterminatedArg {
                            tool: String::from_utf8_lossy(&curr_tag_name).to_string(),
                            arg: String::from_utf8_lossy(&curr_arg_name).to_string(),
                        });
                    }

                    else {
                        cursor += 1;
                    }
                }
            },
        }
    }
}

impl ToolCall {
    pub fn parse(kind: ToolKind, args: &HashMap<Vec<u8>, Vec<u8>>) -> Result<ToolCall, ParseError> {
        match kind {
            ToolKind::Read => {
                let path = match parse_string_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("read"),
                            arg: String::from("path"),
                        });
                    },
                };
                let start = match parse_int_arg("read", args, "start", Some(1), None) {
                    Some(Ok(n)) => Some(n as u64),
                    Some(Err(e)) => return Err(e),
                    None => None,
                };
                let end = match parse_int_arg("read", args, "end", Some(1), None) {
                    Some(Ok(n)) => Some(n as u64),
                    Some(Err(e)) => return Err(e),
                    None => None,
                };
                Ok(ToolCall::Read { path, start, end })
            },
            ToolKind::Write => {
                let path = match parse_string_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("write"),
                            arg: String::from("path"),
                        });
                    },
                };
                let mut content = match parse_string_arg(args, "content") {
                    Some(content) => content,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("write"),
                            arg: String::from("content"),
                        });
                    },
                };
                let mode = match parse_string_arg(args, "mode") {
                    Some(mode) => match mode.as_str() {
                        "create" => WriteMode::Create,
                        "truncate" => WriteMode::Truncate,
                        "append" => WriteMode::Append,
                        mode => {
                            return Err(ParseError::InvalidWriteMode(mode.to_string()));
                        },
                    },
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("write"),
                            arg: String::from("mode"),
                        });
                    },
                };

                // This is a very simple and naive heuristic. It resembles my (baehyunsol) style of writing code.
                // 1. There's no newline in the beginning of a file and exactly 1 newline at the end of a file.
                // 2. When appending something to the file, there's should be a newline between the contents.
                content = content.trim().to_string();

                if !content.is_empty() {
                    content = format!("{content}\n");
                }

                Ok(ToolCall::Write { path, mode, content })
            },
            ToolKind::Patch => {
                let path = match parse_string_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("patch"),
                            arg: String::from("path"),
                        });
                    },
                };
                let diff = match parse_diff_arg(args, "diff") {
                    Some(diff) => diff?,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("patch"),
                            arg: String::from("diff"),
                        });
                    },
                };

                Ok(ToolCall::Patch { path, diff })
            },
            ToolKind::Remove => {
                let path = match parse_string_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("remove"),
                            arg: String::from("path"),
                        });
                    },
                };

                Ok(ToolCall::Remove { path })
            },
            ToolKind::Run => {
                let timeout = match parse_int_arg("run", args, "timeout", Some(1), None) {
                    Some(Ok(n)) => Some(n as u64),
                    Some(Err(e)) => return Err(e),
                    None => None,
                };
                let command = match parse_command_arg(args, "command") {
                    Some(Ok(command)) => {
                        if command.iter().any(|arg| arg == "|" || arg == ">" || arg == "<" || arg.starts_with("2>") || arg.starts_with("&")) {
                            return Err(ParseError::NotBash { tried_cd: !command.is_empty() && command[0] == "cd" });
                        }

                        else {
                            command
                        }
                    },
                    Some(Err(e)) => {
                        return Err(e);
                    },
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("run"),
                            arg: String::from("command"),
                        });
                    },
                };
                let path = parse_string_arg(args, "path");
                let env = match parse_env_arg(args, "env") {
                    Some(Ok(env)) => env,
                    Some(Err(e)) => return Err(e),
                    None => vec![],
                };
                let stdout = parse_string_arg(args, "stdout");
                let stderr = parse_string_arg(args, "stderr");
                Ok(ToolCall::Run { timeout, command, path, env, stdout, stderr })
            },
            ToolKind::Ask => {
                let to = match parse_string_arg(args, "to") {
                    Some(to) => match to.as_str() {
                        "user" => AskTo::User,
                        "web" => AskTo::Web,
                        to => {
                            return Err(ParseError::InvalidAskTo(to.to_string()));
                        },
                    },
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("ask"),
                            arg: String::from("to"),
                        });
                    },
                };
                let question = match parse_string_arg(args, "question") {
                    Some(question) => question.to_string(),
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("ask"),
                            arg: String::from("question"),
                        });
                    },
                };

                // The harness implements QuestionKind::Choice, but I haven't written prompt for that.
                let question = QuestionToUser {
                    question,
                    kind: QuestionKind::FreeText,
                };

                Ok(ToolCall::Ask { id: InterruptId::new(), to, question })
            },
            ToolKind::Chrome => {
                let script = parse_string_arg(args, "script");
                let input = match parse_string_arg(args, "input") {
                    Some(input) => input,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("chrome"),
                            arg: String::from("input"),
                        });
                    },
                };
                // TODO: any better heuristics?
                let input = if input.starts_with("http://") || input.starts_with("https://") {
                    WebOrFile::Web(input)
                } else {
                    WebOrFile::File(input)
                };
                let output = match parse_string_arg(args, "output") {
                    Some(output) => output,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("chrome"),
                            arg: String::from("output"),
                        });
                    },
                };
                Ok(ToolCall::Chrome { script, input, output })
            },
            ToolKind::ImageEdit => {
                let input = match parse_string_arg(args, "input") {
                    Some(input) => input,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("image-edit"),
                            arg: String::from("input"),
                        });
                    },
                };
                let prompt = match parse_string_arg(args, "prompt") {
                    Some(prompt) => prompt,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("image-edit"),
                            arg: String::from("prompt"),
                        });
                    },
                };
                let size = match parse_image_size_arg(args, "size") {
                    Some(Ok(size)) => Some(size),
                    Some(Err(e)) => return Err(e),
                    None => None,
                };
                let output = match parse_string_arg(args, "output") {
                    Some(output) => output,
                    None => {
                        return Err(ParseError::MissingArg {
                            tool: String::from("image-edit"),
                            arg: String::from("output"),
                        });
                    },
                };
                Ok(ToolCall::ImageEdit { input, prompt, size, output })
            },
        }
    }
}

fn parse_int_arg(
    tool: &str,
    args: &HashMap<Vec<u8>, Vec<u8>>,
    arg: &str,

    // both are inclusive
    min: Option<i64>,
    max: Option<i64>,
) -> Option<Result<i64, ParseError>> {
    let n = args.get(arg.as_bytes())?;
    let n = String::from_utf8_lossy(n);
    let n = n.parse::<i64>().map_err(|_| ParseError::ArgTypeError { tool: tool.to_string(), expected_type: ArgType::Integer, arg_name: arg.to_string(), arg: n.to_string() });

    if let Ok(n) = n {
        if let Some(min) = min && n < min {
            Some(Err(ParseError::InvalidIntegerRange { tool: tool.to_string(), arg_name: arg.to_string(), min: Some(min), max, n }))
        }

        else if let Some(max) = max && n > max {
            Some(Err(ParseError::InvalidIntegerRange { tool: tool.to_string(), arg_name: arg.to_string(), min, max: Some(max), n }))
        }

        else {
            Some(Ok(n))
        }
    }

    else {
        Some(n)
    }
}

fn parse_string_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<String> {
    args.get(arg.as_bytes()).map(|s| String::from_utf8_lossy(s).to_string())
}

fn parse_command_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Result<Vec<String>, ParseError>> {
    let command = args.get(arg.as_bytes())?;
    let command = String::from_utf8_lossy(command);
    Some(parse_command(&command).map_err(ParseError::InvalidCommand))
}

pub static ENV_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-zA-Z0-9_-]+)\s*\=\s*(.+)").unwrap());

fn parse_env_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Result<Vec<(String, String)>, ParseError>> {
    let env = args.get(arg.as_bytes())?;
    let env = String::from_utf8_lossy(env);
    let mut result = vec![];

    for line in env.lines() {
        if line.is_empty() {
            continue;
        }

        match ENV_REGEX.captures(line) {
            Some(m) => {
                let key = m.get(1).unwrap().as_str().to_string();
                let value = m.get(2).unwrap().as_str().to_string();
                result.push((key, value));
            },
            None => {
                return Some(Err(ParseError::InvalidEnv(line.to_string())));
            },
        }
    }

    Some(Ok(result))
}

fn parse_diff_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Result<Vec<LineDiff>, ParseError>> {
    let lines = args.get(arg.as_bytes())?;
    let lines = String::from_utf8_lossy(lines);
    Some(parse_line_diff(&lines))
}

static SIZE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(\d+)x(\d+)\s*").unwrap());

fn parse_image_size_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Result<(u64, u64), ParseError>> {
    let size = args.get(arg.as_bytes())?;
    let size = String::from_utf8_lossy(size);

    match SIZE_REGEX.captures(&size) {
        Some(cap) => Some(Ok((
            cap.get(1).unwrap().as_str().parse::<u64>().unwrap(),
            cap.get(2).unwrap().as_str().parse::<u64>().unwrap(),
        ))),
        None => Some(Err(ParseError::InvalidImageSize(size.to_string()))),
    }
}

fn check_tag_name(input: &[u8]) -> Option<&[u8]> {
    if let Some(b'<') = input.get(0) {
        // fine
    } else {
        return None;
    }

    let mut i = 1;

    loop {
        match input.get(i) {
            Some(b'a'..=b'z' | b'-' | b'/') => {
                i += 1;
            },
            Some(b'>') => {
                if i > 1 {
                    return Some(&input[1..i]);
                }

                else {
                    return None;
                }
            },
            _ => {
                return None;
            },
        }
    }
}
