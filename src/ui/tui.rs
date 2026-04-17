use super::{FeContext, Truncation, spawn_backend_process};
use crate::{Error, TurnResultSummary, prettify_time};
use std::thread::sleep;
use std::time::Duration;

// TODO: don't refresh the terminal if there's no update
pub fn run(no_backend: bool) -> Result<(), Error> {
    if !no_backend {
        spawn_backend_process()?;
    }

    // Backend might dump error messages to stderr. So we wait here.
    sleep(Duration::from_millis(2000));

    let mut context = FeContext::load()?;
    let mut has_to_erase_terminal = false;
    let mut prev_buffer = vec![];
    let mut curr_buffer = vec![];

    loop {
        prev_buffer = curr_buffer;
        curr_buffer = vec![];
        context.start_frame()?;

        curr_buffer.push(format!("{}", context.top_bar()?));

        for (i, preview) in context.iter_previews().into_iter().enumerate() {
            let truncation = match context.truncation.get(&preview.id).unwrap() {
                Truncation::Hidden => "\x1b[101m   \x1b[0m",
                Truncation::FullRender => "\x1b[102m   \x1b[0m",
                Truncation::ShortRender => "\x1b[104m   \x1b[0m",
            };

            curr_buffer.push(format!(
                "{i:>3}. {truncation}[{}] {}{}\n{}(LLM: {}, TOOL: {})",
                preview.timestamp,
                preview.preview_title,
                match preview.result {
                    TurnResultSummary::ParseError => " \x1b[101m(parse-error)\x1b[0m    ",
                    TurnResultSummary::ToolCallError => " \x1b[103m(tool-call-error)\x1b[0m",
                    TurnResultSummary::ToolCallSuccess => "",
                },
                " ".repeat(35),
                prettify_time(preview.llm_elapsed_ms),
                prettify_time(preview.tool_elapsed_ms),
            ));
        }

        curr_buffer.push(format!("{}", context.curr_status()));

        if let Some(error) = context.curr_error() {
            curr_buffer.push(format!("\x1b[101m{error}\x1b[0m"));
        }

        if curr_buffer != prev_buffer {
            if has_to_erase_terminal {
                println!("\x1b[2J");
                println!("\x1bc");
            }

            println!("{}", curr_buffer.join("\n"));
        }

        sleep(Duration::from_millis(3000));
        // TODO: user interaction
        context.end_frame(None, None)?;
        has_to_erase_terminal = true;
    }
}
