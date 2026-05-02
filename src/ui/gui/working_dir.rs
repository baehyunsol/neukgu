use super::{
    FeContext,
    Truncation,
    black,
    blue,
    button,
    disabled_button,
    gray,
    green,
    pink,
    red,
    set_bg,
    spawn_be_process,
    white,
    yellow,
};
use crate::{
    Error,
    ImageId,
    LLMToken,
    LogEntry,
    LogId,
    Logger,
    ToolCallSuccess,
    Turn,
    TurnId,
    TurnPreview,
    TurnResult,
    TurnResultSummary,
    UserResponse,
    check_snapshot,
    load_log,
    load_logs_tail,
    prettify_time,
    reset_working_dir,
    roll_back_working_dir,
    stringify_llm_tokens,
};
use iced::{Background, Color, ContentFit, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, Id, MouseArea, Row, Scrollable, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{Handle as ImageHandle, Image, Viewer as ImageViewer};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, focus, scroll_to, snap_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    TextEditor,
};
use ragit_fs::{join, join3};
use regex::Regex;
use std::collections::HashSet;
use std::process::Child;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

const HELP_MESSAGE: &str = "
This is a neukgu's working directory.

Neukgu reads files, writes files and runs programs inside the directory in order to
accomplish the job you gave to neukgu.

## Interactions

There are 2 ways to interact with neukgu.

1. Pause / Resume neukgu.
2. Interrupt: you can give extra instructions while neukgu is working.

Neukgu might ask you a question while he's working. Then, you'll see a
question popup. A question has a timeout. If you don't answer the question
for long time, neukgu will assume that you're not available and just continue
his work.

## Context engineering

Below the buttons, you can see a long list of turns. That is the entire trajectory of
neukgu's operations.

It is very important to decide which turns to be included in the LLM's context. The
harness provides some functionality to engineer neukgu's context.

In the top bar, you'll see how much context neukgu is currently using. The denominator is
the limit of the maximum context size, and the numerator is the current context size, excluding
hidden turns. The context size is measured in bytes. Images are treated as a 2048 byte text.

When the context gets to exceed the limit, the harness will hide turns. If a turn is marked red,
the turn is hidden and neukgu cannot see the turn.

You can also manually hide/pin the turns. If you hide a turn, the turn will be hidden regardless
of the harness' context engineering. You can also pin a turn, and the turn will never be hidden
no matter how many turns you have.

If a turn is marked green or blue, the turn is in the neukgu's context. If it's green, the entire
turn, including LLM's thoughts are included in the context. If it's blue, LLM's thoughts are not
in the context. You can't control this. Only harness can do.

You can also use C key to toggle a turn's visibility.

## Done

When neukgu finishes his job, he'll create `logs/done` file and go to sleep. If you're
not satisfied with his work, you can interrupt him to do more work.
He'll remove `logs/done` file and do more work.
";

pub struct IcedContext {
    pub be_process: Option<Child>,
    pub fe_context: FeContext,
    pub window_size: Size,
    pub turn_view_id: Id,
    pub logs_view_id: Id,
    pub text_editor_id: Id,
    pub turn_view_scrolled: AbsoluteOffset,

    // hovered_turn: mouse
    // selected_turn: arrow keys
    pub hovered_turn: Option<TurnId>,
    pub selected_turn: Option<usize>,

    pub loaded_turn: Option<(usize, Turn)>,
    pub loaded_logs: Option<Vec<String>>,
    pub loaded_image: Option<ImageId>,
    pub user_response_timeout_counter: Instant,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub zoom: f32,
    pub text_editor_content: TextEditorContent,
    pub syntax_highlight: Option<String>,

    // If it's set, it'll display "diff" button in the turn popup.
    pub text_diff: Option<String>,

    // If it's set, it'll display "Open in browser" button in the turn popup.
    pub turn_result_path: Option<(String, Option<String>)>,  // (dir, basename of file)

    // user interaction
    pub is_paused: bool,
    pub pause: Option<bool>,
    pub question_from_user: Option<(u64, String)>,
    pub llm_request: Option<(u64, String)>,
    pub processed_llm_requests: HashSet<u64>,
    pub user_response: Option<(u64, UserResponse)>,
}

impl IcedContext {
    // It returns a scroll-offset of the turn view.
    pub fn select_turn(&mut self, offset: i32) -> f32 {
        let new_selection = (self.selected_turn.map(|i| i as i32).unwrap_or(-1) + offset).min(self.fe_context.history.len() as i32 - 1).max(0) as usize;
        self.selected_turn = Some(new_selection);
        self.zoom * (new_selection.max(3) - 3) as f32 * 61.0
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::Turn(index, turn_id) => {
                let turn = Turn::load(&turn_id, &self.fe_context.working_dir)?;

                if let TurnResult::ToolCallSuccess(ToolCallSuccess::Write { diff: Some(diff), .. }) = &turn.turn_result {
                    self.text_diff = Some(diff.to_string());
                }

                else {
                    self.text_diff = None;
                }

                self.turn_result_path = turn.get_result_path()?;
                self.turn_result_path = self.turn_result_path.as_ref().map(|(dir, file)| (join(&self.fe_context.working_dir, dir).unwrap(), file.clone()));
                self.copy_buffer = Some(format!(
"# {index}. {}

<|LLM|>

{}

<|result|>

{}",
                    turn.preview().preview_title,
                    turn.raw_response,
                    stringify_llm_tokens(&turn.turn_result.to_llm_tokens(&self.fe_context.config)),
                ));
                self.loaded_turn = Some((index, turn));
            },
            // There's nothing to load
            Popup::Interrupt => {},
            Popup::Logs => {
                let log_dir = join3(&self.fe_context.working_dir, ".neukgu", "logs")?;
                let logs = load_logs_tail(&log_dir)?;
                self.copy_buffer = Some(logs.join("\n"));
                self.loaded_logs = Some(logs);
            },
            Popup::Log(id) => {
                let log_dir = join3(&self.fe_context.working_dir, ".neukgu", "logs")?;
                let (mut log, mut extension) = load_log(&id, &log_dir)?;
                self.copy_buffer = Some(log.to_string());

                if log.len() > 16384 {
                    log = String::from("The log is too long to display. Copy the log and paste it to your text editor to see the log.");
                    extension = String::from("txt");
                }

                self.set_text_editor_content(log.to_string());
                self.syntax_highlight = Some(extension);
            },
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_text_editor_content(HELP_MESSAGE.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::Image(id) => {
                self.loaded_image = Some(id);
            },
            // It's already loaded in `self.text_diff`
            Popup::Diff => {
                self.copy_buffer = self.text_diff.clone();
            },
            Popup::TokenUsage => {
                let token_usage = self.fe_context.get_token_usage()?;
                self.set_text_editor_content(token_usage.to_string());
                self.copy_buffer = Some(token_usage.to_string());
            },
            Popup::Instruction => {
                let instruction = self.fe_context.get_instruction()?;
                self.set_text_editor_content(instruction.to_string());
                self.copy_buffer = Some(instruction.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::Config => {
                let config = serde_json::to_string_pretty(&self.fe_context.config)?;
                self.set_text_editor_content(config.to_string());
                self.copy_buffer = Some(config.to_string());
                self.syntax_highlight = Some(String::from("json"));
            },
            Popup::Reset => {
                self.set_text_editor_content(self.fe_context.get_instruction()?);
                self.copy_buffer = None;
                self.syntax_highlight = None;
            },
            Popup::AskRollBack { .. } => {},
            Popup::AskQuit => {},
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
        self.hovered_turn = None;
        self.loaded_turn = None;
        self.loaded_logs = None;
        self.loaded_image = None;
        self.curr_popup = None;
        self.copy_buffer = None;
        self.text_editor_content = TextEditorContent::with_text("");
        self.syntax_highlight = None;
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.text_editor_content.perform(TextEditorAction::SelectAll);
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn kill_be_process(&mut self) -> Result<(), Error> {
        match &mut self.be_process {
            Some(be) => {
                be.kill()?;
                self.be_process = None;
                let logger = Logger::new(&self.fe_context.working_dir);
                logger.log(LogEntry::KillBackend)?;
            },
            None => {},
        }

        Ok(())
    }

    pub fn spawn_be_process(&mut self) -> Result<(), Error> {
        if self.be_process.is_none() {
            self.be_process = Some(spawn_be_process(&self.fe_context.working_dir)?);
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    TurnViewScrolled(AbsoluteOffset),
    HoverOnTurn(Option<TurnId>),
    OpenPopup {
        curr: Popup,
        prev: Option<Popup>,
    },
    BackPopup,
    ClosePopup,
    CopyToClipboard,
    ToggleTurnVisibility(TurnId),
    PauseNeukgu,
    ResumeNeukgu,
    InterruptNeukgu,
    ResetNeukgu,
    RollBackNeukgu(TurnId),
    AnswerLLMRequest,
    DismissLLMRequest,
    EditText(TextEditorAction),
    OpenBrowser { dir: String, file: Option<String> },
    Error(String),
    Quit,
}

#[derive(Clone, Debug)]
pub enum Popup {
    Turn(usize, TurnId),
    Interrupt,
    Logs,
    Log(LogId),
    Help,
    Image(ImageId),
    Diff,
    TokenUsage,
    Instruction,
    Config,
    Reset,
    AskRollBack { id: TurnId, title: String },
    AskQuit,
}

pub fn try_boot(no_backend: bool, working_dir: &str, window_size: Size, zoom: f32) -> Result<IcedContext, Error> {
    let fe_context = FeContext::load(working_dir)?;
    let be_process = if no_backend { None } else { Some(spawn_be_process(working_dir)?) };
    Ok(IcedContext {
        be_process,
        fe_context: fe_context.clone(),
        window_size,
        turn_view_id: Id::unique(),
        logs_view_id: Id::unique(),
        text_editor_id: Id::unique(),
        turn_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
        hovered_turn: None,
        selected_turn: None,
        loaded_turn: None,
        loaded_logs: None,
        loaded_image: None,
        user_response_timeout_counter: Instant::now(),
        curr_popup: None,
        prev_popup: None,
        copy_buffer: None,
        zoom,
        text_editor_content: TextEditorContent::new(),
        syntax_highlight: None,
        text_diff: None,
        turn_result_path: None,
        is_paused: fe_context.is_paused()?,
        pause: None,
        question_from_user: None,
        llm_request: None,
        processed_llm_requests: HashSet::new(),
        user_response: None,
    })
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick => {
            context.fe_context.end_frame(
                context.pause.take(),
                context.question_from_user.take(),
                context.user_response.take(),
            )?;

            if context.loaded_logs.is_some() {
                let log_dir = join3(&context.fe_context.working_dir, ".neukgu", "logs")?;
                let logs = load_logs_tail(&log_dir)?;
                context.copy_buffer = Some(logs.join("\n"));
                context.loaded_logs = Some(logs);
            }

            if let Some(Popup::TokenUsage) = &context.curr_popup {
                let token_usage = context.fe_context.get_token_usage()?;
                context.set_text_editor_content(token_usage.to_string());
                context.copy_buffer = Some(token_usage.to_string());
            }

            let llm_request = context.fe_context.get_llm_request()?;

            if let Some((id, _)) = &llm_request {
                if !context.processed_llm_requests.contains(id) {
                    if context.llm_request.is_none() {
                        context.close_popup();
                        context.user_response_timeout_counter = Instant::now();
                    }

                    context.llm_request = llm_request;
                }
            }

            else if context.llm_request.is_some() {
                context.llm_request = None;
                context.close_popup();
            }

            context.is_paused = context.fe_context.is_paused()?;
            context.fe_context.start_frame()?;
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Backspace), false, false, false) => {
                if context.curr_popup.is_some() && context.prev_popup.is_some() {
                    return Ok(Task::done(IcedMessage::BackPopup));
                }
            },
            (Key::Named(NamedKey::Escape), false, false, false) => {
                // It shouldn't close the llm request popup.
                if context.llm_request.is_none() {
                    if context.curr_popup.is_some() {
                        return Ok(Task::done(IcedMessage::ClosePopup));
                    }

                    else {
                        return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }));
                    }
                }
            },
            (Key::Named(NamedKey::ArrowUp), ctrl, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    let scroll_index = context.select_turn(if ctrl { -10 } else { -1 });
                    return Ok(scroll_to(context.turn_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }
            },
            (Key::Named(NamedKey::ArrowDown), ctrl, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    let scroll_index = context.select_turn(if ctrl { 10 } else { 1 });
                    return Ok(scroll_to(context.turn_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }
            },
            (Key::Named(NamedKey::ArrowLeft), false, false, false) => {
                if let Some(Popup::Turn(index @ 1.., _)) = context.curr_popup {
                    let new_index = index - 1;
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Turn(new_index, context.fe_context.history[new_index].id.clone()), prev: None }));
                }
            },
            (Key::Named(NamedKey::ArrowRight), false, false, false) => {
                if let Some(Popup::Turn(index, _)) = context.curr_popup {
                    let new_index = index + 1;

                    if new_index < context.fe_context.history.len() {
                        return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Turn(new_index, context.fe_context.history[new_index].id.clone()), prev: None }));
                    }
                }
            },
            (Key::Named(NamedKey::Enter), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() && let Some(i) = context.selected_turn {
                    match context.fe_context.history.get(i) {
                        Some(turn) => {
                            return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Turn(i, turn.id.clone()), prev: None }));
                        },
                        None => {},
                    }
                }
            },
            (Key::Named(NamedKey::Space), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    if context.is_paused {
                        return Ok(Task::done(IcedMessage::ResumeNeukgu));
                    } else {
                        return Ok(Task::done(IcedMessage::PauseNeukgu));
                    }
                }
            },
            (Key::Character("c"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    if let Some(i) = context.selected_turn && let Some(turn) = &context.fe_context.history.get(i) {
                        return Ok(Task::done(IcedMessage::ToggleTurnVisibility(turn.id.clone())));
                    }
                }
            },
            (Key::Character("d"), false, false, false) => {
                if let Some(Popup::Turn(_, _)) = &context.curr_popup && context.text_diff.is_some() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Diff, prev: context.curr_popup.clone() }));
                }
            },
            (Key::Character("h"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Help, prev: None }));
                }
            },
            (Key::Character("i"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Interrupt, prev: None }));
                }
            },
            (Key::Character("l"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }));
                }
            },
            (Key::Character("o"), false, false, false) => {
                if let Some(Popup::Turn(_, _)) = &context.curr_popup && let Some((dir, file)) = &context.turn_result_path {
                    return Ok(Task::done(IcedMessage::OpenBrowser { dir: dir.to_string(), file: file.clone() }));
                }
            },
            (Key::Character("r"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Reset, prev: None }));
                }
            },
            (Key::Character("q"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }));
                }
            },
            (Key::Character("t"), false, false, false) => {
                if context.curr_popup.is_none() && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }));
                }
            },
            (Key::Character("y"), false, false, false) => {
                if let Some(Popup::AskRollBack { id, .. }) = &context.curr_popup && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::RollBackNeukgu(id.clone())));
                }

                if let Some(Popup::AskQuit) = context.curr_popup && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::Quit));
                }
            },
            (Key::Character("-"), true, false, false) => {
                context.zoom = context.zoom.max(0.2) - 0.1;
            },
            (Key::Character("="), true, false, false) => {
                context.zoom = context.zoom.min(2.4) + 0.1;
            },
            _ => {},
        },
        IcedMessage::TurnViewScrolled(o) => {
            context.turn_view_scrolled = o;
        },
        IcedMessage::HoverOnTurn(id) => {
            context.hovered_turn = id;
        },
        IcedMessage::OpenPopup { curr, prev } => {
            let mut tasks: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled),
                focus(context.text_editor_id.clone()),
            ];

            if let Popup::Logs = &curr {
                tasks.push(snap_to(context.logs_view_id.clone(), RelativeOffset::END));
            }

            context.open_popup(curr)?;
            context.prev_popup = prev;
            return Ok(Task::batch(tasks));
        },
        IcedMessage::BackPopup => {
            if let Some(prev_popup) = &context.prev_popup {
                let prev_popup = prev_popup.clone();
                context.open_popup(prev_popup)?;
                context.prev_popup = None;
            }
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled));
        },
        IcedMessage::CopyToClipboard => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        // "" -> "hidden" -> "pinned"
        IcedMessage::ToggleTurnVisibility(id) => {
            match (context.fe_context.hidden_turns.remove(&id), context.fe_context.pinned_turns.remove(&id)) {
                (true, _) => {
                    context.fe_context.pinned_turns.insert(id);
                },
                (_, true) => {},
                (false, false) => {
                    context.fe_context.hidden_turns.insert(id);
                },
            }

            context.fe_context.interrupt_be()?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::PauseNeukgu => {
            context.pause = Some(true);
            context.fe_context.interrupt_be()?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::ResumeNeukgu => {
            context.pause = Some(false);
        },
        IcedMessage::InterruptNeukgu => {
            context.question_from_user = Some((rand::random::<u64>(), context.text_editor_content.text()));
            context.close_popup();
            context.fe_context.interrupt_be()?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::ResetNeukgu => {
            context.kill_be_process()?;
            reset_working_dir(context.text_editor_content.text(), &context.fe_context.working_dir)?;
            context.spawn_be_process()?;
            context.fe_context = FeContext::load(&context.fe_context.working_dir)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::RollBackNeukgu(id) => {
            context.kill_be_process()?;
            context.close_popup();

            // There's a chance that the snapshot is removed while the user was looking at `Popup::AskRollBack`.
            if !check_snapshot(&id, &context.fe_context.working_dir)? {
                return Err(Error::CannotFindSnapshot(id.clone()));
            }

            else {
                roll_back_working_dir(&id, &context.fe_context.working_dir)?;
            }

            context.spawn_be_process()?;
            context.fe_context = FeContext::load(&context.fe_context.working_dir)?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::AnswerLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Answer(context.text_editor_content.text())));
            context.text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::DismissLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Reject));
            context.text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::EditText(a) => {
            context.text_editor_content.perform(a);
        },
        IcedMessage::OpenBrowser { .. } => unreachable!(),
        IcedMessage::Error(_) => unreachable!(),
        IcedMessage::Quit => {
            context.kill_be_process()?;
            // TODO: close tab
        },
    }

    Ok(Task::none())
}

pub fn view<'a>(context: &'a IcedContext) -> Element<'a, IcedMessage> {
    let mut turns: Vec<Element<IcedMessage>> = context.fe_context.iter_previews().into_iter().enumerate().map(
        |(i, p)| render_turn_preview(i, &p, context)
    ).collect();

    turns.push(text!("{}", context.fe_context.curr_status()).size(context.zoom * 14.0).into());

    if let Some(error) = context.fe_context.curr_error() {
        turns.push(text!("{error}").size(context.zoom * 14.0).color(red()).into());
    }

    // It makes rooms for popups when there're not enough turns.
    turns.push(text!("").width(context.window_size.width).height(context.window_size.height).into());

    let turns_stretched = Column::from_vec(turns)
        .padding(context.zoom * 8.0)
        .spacing(context.zoom * 8.0);

    let mut turns_scrollable = Scrollable::new(turns_stretched).id(context.turn_view_id.clone());

    if context.curr_popup.is_none() && context.llm_request.is_none() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::TurnViewScrolled(v.absolute_offset()));
    }

    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));
    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.fe_context.top_bar()).size(context.zoom * 14.0)).padding(context.zoom * 8.0).into(),
        render_buttons(context),
        turns_colored.into(),
    ]);

    let mut full_view_stacked: Element<IcedMessage> = Container::new(full_view).into();

    if context.llm_request.is_some() {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_ask_to_user_popup(context),
        ]).into();
    }

    else if let Some((index, turn)) = &context.loaded_turn {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_turn(*index, turn, context),
        ]).into();
    }

    else if let Some(logs) = &context.loaded_logs {
        let view = render_logs(logs, context);
        full_view_stacked = Stack::from_vec(vec![full_view_stacked, view]).into();
    }

    else if let Some(loaded_image) = context.loaded_image {
        let image_view: Element<_> = popup(
            ImageViewer::new(ImageHandle::from_path(loaded_image.path(&context.fe_context.working_dir).unwrap())).content_fit(ContentFit::Contain).into(),
            context,
        ).into();

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            image_view,
        ]).into();
    }

    // TODO: we can use the syntax highlighter
    else if let Some(Popup::Diff) = context.curr_popup {
        let diff_view = Column::from_vec(context.text_diff.as_ref().unwrap().lines().map(
            |line| {
                let color = match line.chars().next() {
                    Some('+') => green(),
                    Some('-') => red(),
                    Some('@') => yellow(),
                    _ => white(),
                };

                text!("{line}").size(context.zoom * 14.0).color(color).into()
            }
        ).collect());
        let diff_view = Scrollable::new(diff_view);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(diff_view.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Interrupt) = context.curr_popup {
        let text_editor = TextEditor::new(&context.text_editor_content)
            .placeholder("Say something to neukgu!")
            .size(context.zoom * 14.0)
            .id(context.text_editor_id.clone())
            .on_action(|action| IcedMessage::EditText(action));
        let interrupt_edit = Column::from_vec(vec![
            text_editor.into(),
            button("Send", IcedMessage::InterruptNeukgu, green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(interrupt_edit.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Reset) = context.curr_popup {
        let text_editor = TextEditor::new(&context.text_editor_content)
            .placeholder("What do you want neukgu to do?")
            .size(context.zoom * 14.0)
            .id(context.text_editor_id.clone())
            .on_action(|action| IcedMessage::EditText(action));
        let reset_edit = Column::from_vec(vec![
            text!("New instruction").size(context.zoom * 14.0).into(),
            text_editor.into(),
            button("Reset", IcedMessage::ResetNeukgu, green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(reset_edit.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::AskRollBack { id, title }) = &context.curr_popup {
        let q = Column::from_vec(vec![
            text!("Roll back to {title}?").size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::RollBackNeukgu(id.clone()), green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(q.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::AskQuit) = context.curr_popup {
        let q = Column::from_vec(vec![
            text!("Quit session?").size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::Quit, green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(q.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Log(_) | Popup::Help | Popup::TokenUsage | Popup::Instruction | Popup::Config) = &context.curr_popup {
        let text_editor = TextEditor::new(&context.text_editor_content).size(context.zoom * 14.0).highlight(
            &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
            iced::highlighter::Theme::SolarizedDark,
        );

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(Scrollable::new(text_editor).width(Length::Fill).into(), context),
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    if context.curr_popup.is_some() || context.llm_request.is_some() {
        return Container::new(text!("")).padding(context.zoom * 8.0).into();
    }

    let mut buttons_row1: Vec<Element<IcedMessage>> = if context.is_paused {
        vec![button("Resume", IcedMessage::ResumeNeukgu, blue(), context.zoom).into()]
    } else {
        vec![button("Pause", IcedMessage::PauseNeukgu, blue(), context.zoom).into()]
    };
    let mut buttons_row2: Vec<Element<IcedMessage>> = vec![];

    buttons_row1.push(button("(Q)uit", IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }, red(), context.zoom).into());
    buttons_row1.push(button("(I)nterrupt", IcedMessage::OpenPopup { curr: Popup::Interrupt, prev: None }, blue(), context.zoom).into());
    buttons_row1.push(button("See (l)ogs", IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }, blue(), context.zoom).into());
    buttons_row1.push(button("(T)oken usage", IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }, blue(), context.zoom).into());
    buttons_row1.push(button("(H)elp", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink(), context.zoom).into());

    buttons_row2.push(button("Instruction", IcedMessage::OpenPopup { curr: Popup::Instruction, prev: None }, green(), context.zoom).into());
    buttons_row2.push(button("Config", IcedMessage::OpenPopup { curr: Popup::Config, prev: None }, green(), context.zoom).into());
    buttons_row2.push(button("(R)eset", IcedMessage::OpenPopup { curr: Popup::Reset, prev: None }, blue(), context.zoom).into());

    Column::from_vec(vec![
        Row::from_vec(buttons_row1).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
        Row::from_vec(buttons_row2).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
    ]).into()
}

fn render_turn_preview<'t, 'c, 'm>(index: usize, p: &'t TurnPreview, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let roll_back = {
        let (color, text, mut enabled) = if context.fe_context.snapshots.contains(&p.id) {
            (red(), "R", true)
        } else {
            (gray(0.2), " ", false)
        };

        if context.curr_popup.is_some() || context.llm_request.is_some() {
            enabled = false;
        }

        if enabled {
            button(text, IcedMessage::OpenPopup { curr: Popup::AskRollBack { id: p.id.clone(), title: format!("{index:>3}. [{}] {}", p.timestamp, p.preview_title_truncated) }, prev: None }, color, context.zoom)
        } else {
            disabled_button(text, color, context.zoom)
        }
    };

    let context_engineering = {
        let color = match context.fe_context.truncation.get(&p.id).unwrap() {
            Truncation::Hidden => red(),
            Truncation::FullRender => green(),
            Truncation::ShortRender => blue(),
        };
        let text = match (context.fe_context.hidden_turns.get(&p.id), context.fe_context.pinned_turns.get(&p.id)) {
            (Some(_), _) => "hidden",
            (_, Some(_)) => "pinned",
            (None, None) => "      ",
        };

        if context.curr_popup.is_none() && context.llm_request.is_none() {
            button(text, IcedMessage::ToggleTurnVisibility(p.id.clone()), color, context.zoom)
        } else {
            disabled_button(text, color, context.zoom)
        }
    };

    let turn_result: Element<IcedMessage> = match p.result {
        TurnResultSummary::ParseError => text!(" (parse-error)").size(context.zoom * 14.0).color(red()),
        TurnResultSummary::ToolCallError => text!(" (tool-call-error)").size(context.zoom * 14.0).color(yellow()),
        TurnResultSummary::ToolCallSuccess => text!(""),
    }.into();

    let turn_row = Row::from_vec(vec![
        text!("{index:>3}. ").size(context.zoom * 14.0).into(),
        text!("[{}]", p.timestamp).size(context.zoom * 14.0).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{}", p.preview_title_truncated).size(context.zoom * 14.0).into(),
                turn_result,
            ]).into(),
            text!("(LLM: {}, TOOL: {})", prettify_time(p.llm_elapsed_ms), prettify_time(p.tool_elapsed_ms)).width(Length::FillPortion(2)).size(context.zoom * 14.0).into(),
        ]).width(Length::Fill).into(),
    ]).width(Length::Fill).align_y(Vertical::Center).spacing(context.zoom * 4.0);

    let mut with_color = Container::new(turn_row).padding(context.zoom * 8.0);

    if let Some(id) = &context.hovered_turn && &p.id == id {
        with_color = with_color.style(|_| set_bg(gray(0.45)));
    }

    else {
        with_color = with_color.style(|_| set_bg(gray(0.15)));
    }

    let with_mouse_area: Element<IcedMessage> = if context.curr_popup.is_none() && context.llm_request.is_none() {
        MouseArea::new(with_color)
            .on_enter(IcedMessage::HoverOnTurn(Some(p.id.clone())))
            .on_exit(IcedMessage::HoverOnTurn(None))
            .on_press(IcedMessage::OpenPopup { curr: Popup::Turn(index, p.id.clone()), prev: None })
            .into()
    }

    else {
        with_color.into()
    };

    let mut result = vec![];

    if let Some(i) = context.selected_turn && i == index {
        result.push(text!(">> ").size(context.zoom * 14.0).into());
    }

    result.extend(vec![roll_back.into(), context_engineering.into(), with_mouse_area]);

    Row::from_vec(result)
        .width(Length::Fixed(context.window_size.width))
        .align_y(Vertical::Center)
        .spacing(context.zoom * 12.0)
        .into()
}

fn render_turn<'a, 'b, 'c>(index: usize, turn: &'a Turn, context: &'b IcedContext) -> Element<'c, IcedMessage> {
    let mut turn_content = vec![
        text!("# {index}. {}", turn.preview().preview_title).size(context.zoom * 14.0).into(),
        text!("<|LLM|>").size(context.zoom * 14.0).into(),
        Container::new(
            render_llm_tokens(vec![LLMToken::String(turn.raw_response.to_string())], context)
        ).padding(context.zoom * 8.0).style(|_| set_bg(gray(0.3))).into(),
        text!("<|result|>").size(context.zoom * 14.0).into(),
        Container::new(
            render_llm_tokens(turn.turn_result.to_llm_tokens(&context.fe_context.config), context)
        ).padding(context.zoom * 8.0).style(|_| set_bg(gray(0.3))).into(),
    ];

    let mut buttons = vec![];

    if context.text_diff.is_some() {
        buttons.push(button(
            "(D)iff",
            IcedMessage::OpenPopup { curr: Popup::Diff, prev: context.curr_popup.clone() },
            green(),
            context.zoom,
        ).into());
    }

    if let Some((dir, file)) = &context.turn_result_path {
        buttons.push(button(
            "(O)pen in browser",
            IcedMessage::OpenBrowser { dir: dir.to_string(), file: file.clone() },
            green(),
            context.zoom,
        ).into());
    }

    if !buttons.is_empty() {
        turn_content.push(Row::from_vec(buttons).into());
    }

    let turn_content = Scrollable::new(Column::from_vec(turn_content).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).width(Length::Fill)).width(Length::Fill);
    popup(turn_content.into(), context)
}

pub static LOG_DETAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*\((\d{7}\-\d{7})\).*").unwrap());

fn render_logs<'a, 'b, 'c>(logs: &'a [String], context: &'b IcedContext) -> Element<'c, IcedMessage> {
    let logs = Scrollable::new(Column::from_vec(
        logs.iter().map(
            |log| {
                if let Some(cap) = LOG_DETAIL_RE.captures(log) {
                    let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                    Row::from_vec(vec![
                        text!("{log}").size(context.zoom * 14.0).into(),
                        button("see details", IcedMessage::OpenPopup {
                            curr: Popup::Log(log_id),
                            prev: Some(Popup::Logs),
                        }, green(), context.zoom).into(),
                    ]).align_y(Vertical::Center).spacing(context.zoom * 20.0).into()
                }

                else {
                    text!("{log}").size(context.zoom * 14.0).into()
                }
            }
        ).collect()
    ).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).width(Length::Fill)).id(context.logs_view_id.clone()).width(Length::Fill);
    popup(logs.into(), context)
}

fn render_llm_tokens(llm_tokens: Vec<LLMToken>, context: &IcedContext) -> Element<'static, IcedMessage> {
    Column::from_vec(llm_tokens.iter().map(
        |token| match token {
            LLMToken::String(s) => text!("{s}").size(context.zoom * 14.0).width(Length::Fill).into(),
            LLMToken::Image(id) => MouseArea::new(
                Image::new(ImageHandle::from_path(id.path(&context.fe_context.working_dir).unwrap()))
                    .width(Length::Fixed(300.0))
                    .height(Length::Fixed(300.0))
                    .content_fit(ContentFit::Contain),
            ).on_press(
                IcedMessage::OpenPopup {
                    curr: Popup::Image(*id),
                    prev: context.curr_popup.clone(),
                },
            ).into(),
        }
    ).collect()).width(Length::Fill).into()
}

fn render_ask_to_user_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let elapsed_secs = Instant::now().duration_since(context.user_response_timeout_counter.clone()).as_secs();

    popup(
        Column::from_vec(vec![
            text!("{}", context.llm_request.as_ref().unwrap().1).size(context.zoom * 14.0).into(),
            TextEditor::new(&context.text_editor_content)
                .placeholder("Answer neukgu's question")
                .size(context.zoom * 14.0)
                .width(context.window_size.width - context.zoom * 128.0)
                .on_action(|action| IcedMessage::EditText(action))
                .into(),
            Row::from_vec(vec![
                button("Answer", IcedMessage::AnswerLLMRequest, green(), context.zoom).into(),
                button("Dismiss", IcedMessage::DismissLLMRequest, red(), context.zoom).into(),
                text!("{}", context.fe_context.config.user_response_timeout.max(elapsed_secs) - elapsed_secs).size(context.zoom * 14.0).into(),
            ]).spacing(context.zoom * 20.0).into(),
        ]).padding(context.zoom * 20.0).spacing(context.zoom * 20.0).into(),
        context,
    )
}

fn popup<'e, 'c>(element: Element<'e, IcedMessage>, context: &'c IcedContext) -> Element<'e, IcedMessage> {
    let mut buttons: Vec<Element<IcedMessage>> = vec![];

    if context.curr_popup.is_some() {
        buttons.push(button("Close", IcedMessage::ClosePopup, red(), context.zoom).into());
    }

    if context.prev_popup.is_some() {
        buttons.push(button("Back", IcedMessage::BackPopup, blue(), context.zoom).into());
    }

    if context.copy_buffer.is_some() {
        buttons.push(button("Copy", IcedMessage::CopyToClipboard, blue(), context.zoom).into());
    }

    Container::new(
        Container::new(Column::from_vec(vec![
            Row::from_vec(buttons).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
            element,
        ]).width(Length::Fill)).style(
            |_| Style {
                background: Some(Background::Color(black())),
                border: Border {
                    color: white(),
                    width: 4.0,
                    radius: Radius::new(8.0),
                },
                ..Style::default()
            }
        )
        .padding(context.zoom * 8.0)
        .width(Length::Fill)
    )
    .style(|_| set_bg(Color::from_rgba(0.0, 0.0, 0.0, 0.5)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(context.zoom * 32.0)
    .into()
}
