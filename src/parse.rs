use crate::{AskTo, StringOrImage, ToolCall, ToolKind};
use crate::tool::WriteMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ParsedSegment {
    String(String),
    ToolCall {
        call: ToolCall,
        input: String,
    },
}

pub fn get_first_tool_call(r: &[ParsedSegment]) -> Option<&ParsedSegment> {
    for s in r.iter() {
        if let ParsedSegment::ToolCall { .. } = s {
            return Some(s);
        }
    }

    None
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
    NoTag,
    InvalidTag(String),
    MultipleTags,
    WrongArg {
        tag: String,
        arg: String,
    },
    MissingArg {
        tag: String,
        arg: String,
    },
    UnterminatedArg {
        tag: String,
        arg: String,
    },
    InvalidWriteMode(String),
    NotBash,
    InvalidAskTo(String),
}

impl ParseError {
    pub fn to_llm_tokens(&self) -> Vec<StringOrImage> {
        match self {
            ParseError::NoTag => vec![StringOrImage::String(String::from("
I can't find any XML-syntaxed tool calls in your response.
Please call a tool.
"))],
            // Why the fuck is claude calling multiple tools in a single turn?
            ParseError::MultipleTags => vec![StringOrImage::String(String::from("
Failed to call tools.

You tried to call multiple tools in a single turn. I see multiple XML syntaxes in your response.
NONE OF YOUR ACTIONS IN YOUR PREVIOUS TURN WAS RUN.
You can call exactly 1 tool per turn. You have to call exactly 1 tool per turn, do you understand?
I repeat, just call a single tool and finish your turn.
            "))],
            ParseError::NotBash => vec![StringOrImage::String(String::from("
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
</run>
"))],
            _ => panic!("TODO: {self:?}"),
        }
    }
}

pub fn validate_parse_result(parse_result: &[ParsedSegment]) -> Result<ToolCall, ParseError> {
    for segment in parse_result.iter() {
        if let ParsedSegment::ToolCall { call, .. } = segment {
            return Ok(call.clone())
        }
    }

    // NOTE: `parse` already checked that there's only 1 tag
    // TODO: any other checks to do?
    unreachable!()
}

pub fn parse(input: &[u8]) -> Result<Vec<ParsedSegment>, ParseError> {
    let mut cursor = 0;
    let mut segments = vec![];
    let mut buffer = vec![];
    let mut state = ParseState::String;

    let mut tool_call_start = 0;
    let mut curr_tag_name = vec![];
    let mut curr_arg_name = vec![];
    let mut args = HashMap::new();
    let mut valid_tag_count = 0;

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
                        Some(t @ (b"read" | b"write" | b"run" | b"ask" | b"render")) => {
                            let tool_kind = match t {
                                b"read" => ToolKind::Read,
                                b"write" => ToolKind::Write,
                                b"run" => ToolKind::Run,
                                b"ask" => ToolKind::Ask,
                                b"render" => ToolKind::Render,
                                _ => unreachable!(),
                            };
                            curr_tag_name = t.to_vec();
                            args = HashMap::new();

                            if !buffer.is_empty() {
                                segments.push(ParsedSegment::String(String::from_utf8_lossy(&buffer).to_string()));
                                buffer = vec![];
                            }

                            cursor += t.len() + 2;  // 2 for '<' and '>'.
                            state = ParseState::Tool { kind: tool_kind, top_level: true };
                        },
                        Some(t) => {
                            if first_tag_but_wrong_name.is_none() {
                                first_tag_but_wrong_name = Some(t.to_vec());
                            }

                            buffer.push(b'<');
                            cursor += 1;
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
                        segments.push(ParsedSegment::String(String::from_utf8_lossy(&buffer).to_string()));
                    }

                    return match valid_tag_count {
                        0 => match first_tag_but_wrong_name {
                            Some(wrong_tag) => Err(ParseError::InvalidTag(String::from_utf8_lossy(&wrong_tag).to_string())),
                            None => Err(ParseError::NoTag),
                        },
                        1 => Ok(segments),
                        _ => Err(ParseError::MultipleTags),
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
                        segments.push(ParsedSegment::ToolCall {
                            call: ToolCall::parse(*kind, &args)?,
                            input: String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string(),
                        });
                        valid_tag_count += 1;
                        state = ParseState::String;
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
                            return Err(ParseError::WrongArg {
                                tag: String::from_utf8_lossy(&curr_tag_name).to_string(),
                                arg: String::from_utf8_lossy(t).to_string(),
                            });
                        }
                    },
                    None => {
                        if first_tag_but_wrong_arg.is_none() {
                            first_tag_but_wrong_arg = Some(curr_tag_name.to_vec());
                        }

                        segments.push(ParsedSegment::String(String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string()));
                        state = ParseState::String;
                    },
                },
                _ => {
                    if first_tag_but_wrong_arg.is_none() {
                        first_tag_but_wrong_arg = Some(curr_tag_name.to_vec());
                    }

                    segments.push(ParsedSegment::String(String::from_utf8_lossy(&input[tool_call_start..cursor]).to_string()));
                    state = ParseState::String;
                },
            },
            ParseState::Tool { kind, top_level: false } => {
                let end_tag = format!("</{}>", String::from_utf8_lossy(&curr_arg_name).to_string());
                let end_tag = end_tag.as_bytes();
                let arg_start = cursor;

                loop {
                    if input.get(cursor..(cursor + end_tag.len())) == Some(end_tag) {
                        args.insert(curr_arg_name.to_vec(), input[arg_start..cursor].to_vec());
                        cursor += end_tag.len();
                        state = ParseState::Tool { kind: *kind, top_level: true };
                        break;
                    }

                    else if cursor > input.len() {
                        return Err(ParseError::UnterminatedArg {
                            tag: String::from_utf8_lossy(&curr_tag_name).to_string(),
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
                let path = match parse_path_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("read"),
                            arg: String::from("path"),
                        });
                    },
                };
                let start = parse_int_arg(args, "start").map(|n| n as u64);
                let end = parse_int_arg(args, "end").map(|n| n as u64);
                Ok(ToolCall::Read { path, start, end })
            },
            ToolKind::Write => {
                let path = match parse_path_arg(args, "path") {
                    Some(path) => path,
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("write"),
                            arg: String::from("path"),
                        });
                    },
                };
                let mut content = match parse_string_arg(args, "content") {
                    Some(content) => content,
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("write"),
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
                            tag: String::from("write"),
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
            ToolKind::Run => {
                let command = match parse_command_arg(args, "command") {
                    Some(command) => {
                        if command.iter().any(|arg| arg == "|" || arg == ">" || arg == "<" || arg.starts_with("2>") || arg.starts_with("&")) {
                            return Err(ParseError::NotBash);
                        }

                        else {
                            command
                        }
                    },
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("run"),
                            arg: String::from("command"),
                        });
                    },
                };
                let timeout = parse_int_arg(args, "timeout").map(|n| n as u64);
                let stdout = parse_path_arg(args, "stdout");
                let stderr = parse_path_arg(args, "stderr");
                Ok(ToolCall::Run { timeout, command, stdout, stderr })
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
                            tag: String::from("ask"),
                            arg: String::from("to"),
                        });
                    },
                };
                let question = match parse_string_arg(args, "question") {
                    Some(question) => question.to_string(),
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("ask"),
                            arg: String::from("question"),
                        });
                    },
                };
                Ok(ToolCall::Ask { id: rand::random::<u64>(), to, question })
            },
            ToolKind::Render => {
                let input = match parse_path_arg(args, "input") {
                    Some(input) => input,
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("render"),
                            arg: String::from("input"),
                        });
                    },
                };
                let output = match parse_path_arg(args, "output") {
                    Some(output) => output,
                    None => {
                        return Err(ParseError::MissingArg {
                            tag: String::from("render"),
                            arg: String::from("output"),
                        });
                    },
                };
                Ok(ToolCall::Render { input, output })
            },
        }
    }
}

fn parse_path_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Vec<String>> {
    let path = args.get(arg.as_bytes())?;
    let path = String::from_utf8_lossy(path);
    Some(path.split("/").map(|s| s.to_string()).collect())
}

fn parse_int_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<i64> {
    let n = args.get(arg.as_bytes())?;
    let n = String::from_utf8_lossy(n);
    n.parse::<i64>().ok()
}

fn parse_string_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<String> {
    args.get(arg.as_bytes()).map(|s| String::from_utf8_lossy(s).to_string())
}

fn parse_command_arg(args: &HashMap<Vec<u8>, Vec<u8>>, arg: &str) -> Option<Vec<String>> {
    let command = args.get(arg.as_bytes())?;
    let command = String::from_utf8_lossy(command);

    let mut buffer = vec![];
    let mut cli = vec![];
    let mut in_quotation = false;

    for ch in command.chars() {
        match ch {
            ' ' => {
                if in_quotation {
                    buffer.push(' ');
                }

                else if !buffer.is_empty() {
                    cli.push(buffer);
                    buffer = vec![];
                }
            },
            '"' => {
                in_quotation = !in_quotation;
            },
            c => {
                buffer.push(c);
            },
        }
    }

    if !buffer.is_empty() {
        cli.push(buffer);
    }

    Some(cli.into_iter().map(|chs| chs.into_iter().collect()).collect())
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
            Some(b'a'..=b'z' | b'/') => {
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
