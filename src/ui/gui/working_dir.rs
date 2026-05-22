use super::{
    FeContext,
    Truncation,
    black,
    blue,
    button,
    disabled_button,
    gray,
    green,
    green_transparent,
    pink,
    red,
    red_transparent,
    set_bg,
    skyblue,
    spawn_be_process,
    white,
    yellow,
};
use super::api_key::{
    self,
    IcedContext as GetApiKeysContext,
    IcedMessage as GetApiKeysMessage,
    get_api_keys_popup,
};
use super::chat::{ChatMessage, chat_ui};
use super::config::{SetProjectConfig, config_ui, set_project_config};
use super::logs::{LogsContext, render_logs};
use super::popup::{PopupContext, PopupMessage, into_popup};
use super::worker::{Job, JobResult};
use crate::{
    Config,
    Error,
    ImageId,
    LLMToken,
    LogEntry,
    LogId,
    Logger,
    Model,
    SessionSummary,
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
    prettify_timestamp,
    reset_working_dir,
    roll_back_working_dir,
    stringify_llm_tokens,
};
use iced::{Background, ContentFit, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Button, Column, Id, MouseArea, Row, Scrollable, Space, Stack, TextInput, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{Handle as ImageHandle, Image, Viewer as ImageViewer};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, focus, is_focused, scroll_to, snap_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Binding,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    KeyPress,
    TextEditor,
};
use ragit_fs::{join, join3};
use regex::Regex;
use std::collections::HashSet;
use std::collections::hash_map::{Entry, HashMap};
use std::process::Child;
use std::sync::Arc;
use std::time::Instant;

mod file_change;

use file_change::{FileChange, render_udiff};

const HELP_MESSAGE: &str = r#"
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

You can also use Ctrl+V key to toggle a turn's visibility.

## Done

When neukgu finishes his job, he'll create `logs/done` file and go to sleep. If you're
not satisfied with his work, you can interrupt him to do more work.
He'll remove `logs/done` file and do more work.

## Reset

You can reset the instruction and restart the session. It resets the session (turns are gone),
but all the files, except `logs/done` are kept.

## Rollback

You can see red "R" buttons on the left of some turns. By clicking that button, the session, not
only the turns but also the files in the working directory will be rollback to that turn.
It doesn't rollback the configs, though.

## Key bindings

- Backspace: to prev popup
- Esc: close popup
- (Ctrl)+Up/Down: prev/next turn entry (Ctrl + move faster)
  - If there's a popup, Ctrl+Up/Down will scroll to top/bottom
- Left/Right: prev/next turn entry (in turn popup)
- Ctrl+Plus/Minus: zoom
- Ctrl+Tab: toggle focus interrupt_text_edit
- Ctrl+Enter: enter text (when long-text-editor is focused)
  - TODO: it only works with interrupt_text_edit now
- Ctrl+C: configs
  - If there's a popup and a copiable content, it copies the content
- Ctrl+D: see diff (in turn popup)
- Ctrl+F: find in page
- Ctrl+G: see file changes
- Ctrl+H: help message
- Ctrl+L: see logs
- Ctrl+O: open browser
- Ctrl+R: reset
- Ctrl+Q: quit
- Ctrl+S: see summaries
- Ctrl+T: new tab
- Ctrl+U: see token usage
- Ctrl+V: toggle visibility of the current turn
- Ctrl+W: close tab
- Ctrl+Y: yes (confirm popup)
- Alt+Num: switch tab
- Enter: select turn entry
- Space: resume/pause
"#;

#[derive(Debug)]
pub struct IcedContext {
    pub be_process: Option<Child>,
    pub api_keys: HashMap<String, String>,
    pub get_api_keys_context: GetApiKeysContext,
    pub fe_context: FeContext,
    pub window_size: Size,
    pub log_dir: String,
    pub turn_view_id: Id,
    pub short_text_editor_id: Id,
    pub long_text_editor_id: Id,
    pub interrupt_text_editor_id: Id,
    pub popup_scroll_id: Id,
    pub turn_view_scrolled: AbsoluteOffset,

    // hovered_turn: mouse
    // selected_turn: arrow keys
    pub hovered_turn: Option<TurnId>,
    pub selected_turn: Option<usize>,

    pub find_pattern: Option<(String, Regex)>,
    pub find_result: HashMap<String, (usize, usize)>,
    pub loaded_turn: Option<(usize, Turn)>,
    pub loaded_logs: Option<Vec<String>>,
    pub loaded_image: Option<ImageId>,
    pub user_response_timeout_counter: Instant,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub zoom: f32,
    pub short_text_editor_content: String,
    pub long_text_editor_content: TextEditorContent,
    pub interrupt_text_editor_content: TextEditorContent,
    pub is_interrupt_text_editor_focused: bool,
    pub is_interrupt_button_hovered: bool,
    pub syntax_highlight: Option<String>,
    pub popup_title: Option<String>,

    // If it's set, it'll display "diff" button in the turn popup.
    pub text_diff: Option<String>,

    // If it's set, it'll display "Open in browser" button in the turn popup.
    pub turn_result_path: Option<(String, Option<String>)>,  // (dir, basename of file)

    // When the user does something with config_ui, this value is changed.
    // When the user clicks the "apply" button, tmp_config is applied to the real config (it takes a few frames).
    pub tmp_config: Config,
    pub has_to_update_config: bool,

    // user interaction
    pub is_paused: bool,
    pub pause: Option<bool>,
    pub question_from_user: Option<(u64, String)>,
    pub llm_request: Option<(u64, String)>,
    pub processed_llm_requests: HashSet<u64>,
    pub user_response: Option<(u64, UserResponse)>,
}

impl IcedContext {
    pub fn new(
        mut no_backend: bool,
        api_keys: HashMap<String, String>,
        working_dir: &str,
        window_size: Size,
        zoom: f32,
    ) -> Result<IcedContext, Error> {
        let fe_context = FeContext::load(working_dir)?;
        let missing_api_keys = get_missing_api_keys(&api_keys, &fe_context.config);
        let mut curr_popup = None;

        if !missing_api_keys.is_empty() {
            curr_popup = Some(Popup::GetApiKeys);
            no_backend = true;
        }

        let be_process = if no_backend {
            None
        } else {
            Some(spawn_be_process(&api_keys, working_dir)?)
        };

        Ok(IcedContext {
            be_process,
            api_keys,
            get_api_keys_context: GetApiKeysContext::new(missing_api_keys),
            fe_context: fe_context.clone(),
            window_size,
            log_dir: join3(&fe_context.working_dir, ".neukgu", "logs")?,
            turn_view_id: Id::unique(),
            short_text_editor_id: Id::unique(),
            long_text_editor_id: Id::unique(),
            popup_scroll_id: Id::unique(),
            interrupt_text_editor_id: Id::unique(),
            turn_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            hovered_turn: None,
            selected_turn: None,
            find_pattern: None,
            find_result: HashMap::new(),
            loaded_turn: None,
            loaded_logs: None,
            loaded_image: None,
            user_response_timeout_counter: Instant::now(),
            curr_popup,
            prev_popup: None,
            copy_buffer: None,
            zoom,
            short_text_editor_content: String::new(),
            long_text_editor_content: TextEditorContent::new(),
            interrupt_text_editor_content: TextEditorContent::new(),
            is_interrupt_text_editor_focused: false,
            is_interrupt_button_hovered: false,
            syntax_highlight: None,
            popup_title: None,
            text_diff: None,
            turn_result_path: None,
            tmp_config: fe_context.config.clone(),
            has_to_update_config: false,
            is_paused: fe_context.is_paused()?,
            pause: None,
            question_from_user: None,
            llm_request: None,
            processed_llm_requests: HashSet::new(),
            user_response: None,
        })
    }

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
            Popup::GetApiKeys => unreachable!(),
            Popup::Turn(index, turn_id) => {
                let turn = Turn::load(&turn_id, &self.fe_context.working_dir)?;

                if let TurnResult::ToolCallSuccess(ToolCallSuccess::Write { diff: Some(diff), .. }) = &turn.turn_result {
                    self.text_diff = Some(diff.to_string());
                }

                else if let TurnResult::ToolCallSuccess(ToolCallSuccess::Patch { diff, .. }) = &turn.turn_result {
                    self.text_diff = Some(diff.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n"));
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

{}

{}",
                    turn.preview().preview_title,
                    turn.render_llm_response(true),
                    stringify_llm_tokens(&turn.turn_result.to_llm_tokens(&self.fe_context.config)),
                    turn.introduce_agents(),
                ));
                self.loaded_turn = Some((index, turn));
            },
            Popup::Logs => {
                let logs = load_logs_tail(&self.log_dir)?;
                self.copy_buffer = Some(logs.join("\n"));
                self.loaded_logs = Some(logs);
            },
            Popup::Log((title, id)) => {
                let (mut log, mut extension) = load_log(&id, &self.log_dir)?;
                self.copy_buffer = Some(log.to_string());

                if log.len() > 32768 {
                    log = String::from("The log is too long to display. Copy the log and paste it to your text editor to see the log.");
                    extension = String::from("txt");
                }

                self.set_long_text_editor_content(log.to_string());
                self.syntax_highlight = Some(extension);
                self.popup_title = Some(title);
            },
            Popup::Summaries => {},
            Popup::Summary(summary) => {
                self.copy_buffer = Some(summary.summary.to_string());
                self.set_long_text_editor_content(summary.summary.to_string());
            },
            Popup::FileChanges(_) => {
                self.update_file_changes()?;
            },
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_long_text_editor_content(HELP_MESSAGE.to_string());
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
                self.set_long_text_editor_content(token_usage.to_string());
                self.copy_buffer = Some(token_usage.to_string());
            },
            Popup::Prompt => {
                let system_prompt = self.fe_context.get_system_prompt();
                self.set_long_text_editor_content(system_prompt.to_string());
                self.copy_buffer = Some(system_prompt.to_string());
                self.syntax_highlight = None;
            },
            Popup::Instruction => {
                let instruction = self.fe_context.get_instruction()?;
                self.set_long_text_editor_content(instruction.to_string());
                self.copy_buffer = Some(instruction.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::Config => {
                self.tmp_config = self.fe_context.config.clone();
            },
            Popup::Reset => {
                self.set_long_text_editor_content(self.fe_context.get_instruction()?);
                self.copy_buffer = None;
                self.syntax_highlight = None;
            },
            Popup::Find { re, .. } => {
                if let Some(re) = re {
                    self.short_text_editor_content = re;
                }
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
        self.short_text_editor_content = String::new();
        self.long_text_editor_content = TextEditorContent::with_text("");
        self.syntax_highlight = None;
        self.popup_title = None;
    }

    pub fn set_long_text_editor_content(&mut self, c: String) {
        self.long_text_editor_content.perform(TextEditorAction::SelectAll);
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn set_interrupt_text_editor_content(&mut self, c: String) {
        self.interrupt_text_editor_content.perform(TextEditorAction::SelectAll);
        self.interrupt_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.interrupt_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn update_find_result(&mut self) {
        self.find_result = HashMap::new();

        if let Some((_, re)) = self.find_pattern.clone() {
            for turn in self.fe_context.iter_previews().iter() {
                if let Some(m) = re.find(&turn.preview_title_truncated) {
                    self.find_result.insert(turn.preview_title_truncated.to_string(), (m.start(), m.end()));
                }
            }
        }
    }

    pub fn kill_be_process(&mut self) -> Result<(), Error> {
        match &mut self.be_process {
            Some(be) => {
                be.kill()?;
                self.be_process = None;
                let logger = Logger::new(self.log_dir.clone(), None, true, true);
                logger.log(LogEntry::KillBackend)?;
            },
            None => {},
        }

        Ok(())
    }

    pub fn spawn_be_process(&mut self) -> Result<(), Error> {
        if self.be_process.is_none() {
            self.be_process = Some(spawn_be_process(&self.api_keys, &self.fe_context.working_dir)?);
        }

        Ok(())
    }

    pub fn can_click_turn_entry(&self) -> bool {
        self.curr_popup.is_none() && self.llm_request.is_none() && !self.is_interrupt_text_editor_focused
    }
}

impl PopupContext for IcedContext {
    fn can_close_popup(&self) -> bool {
        self.llm_request.is_none() && match self.curr_popup {
            Some(Popup::GetApiKeys) | None => false,
            _ => true,
        }
    }

    fn has_prev_popup(&self) -> bool { self.prev_popup.is_some() }
    fn has_something_to_copy(&self) -> bool { self.copy_buffer.is_some() }
    fn zoom(&self) -> f32 { self.zoom }
}

impl ImagePopup for IcedContext {
    type Message = IcedMessage;

    fn open_image_popup(&self, id: ImageId) -> IcedMessage {
        IcedMessage::OpenPopup {
            curr: Popup::Image(id),
            prev: self.curr_popup.clone(),
        }
    }
}

impl LogsContext for IcedContext {
    type Message = IcedMessage;

    fn open_log_popup(&self, log_title: String, log_id: LogId) -> IcedMessage {
        IcedMessage::OpenPopup {
            curr: Popup::Log((log_title, log_id)),
            prev: Some(Popup::Logs),
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    GetApiKeys(GetApiKeysMessage),
    TurnViewScrolled(AbsoluteOffset),
    HoverOnTurn(Option<TurnId>),
    ExpandFileChange(String),
    ExpandAllFileChanges { log: bool, expand: bool },
    OpenPopup {
        curr: Popup,
        prev: Option<Popup>,
    },
    BackPopup,
    ClosePopup,
    CopyPopupContent,
    ToggleTurnVisibility(TurnId),
    PauseNeukgu,
    ResumeNeukgu,
    RespawnBeProcess,
    InterruptNeukgu,
    ResetNeukgu,
    RollBackNeukgu(TurnId),
    SetTmpConfig(SetProjectConfig),
    ApplyTmpConfig,
    Find,
    AnswerLLMRequest,
    DismissLLMRequest,
    EditShortText(String),
    EditLongText(TextEditorAction),
    EditInterruptText(TextEditorAction),
    IsInterruptTextEditorFocused(bool),
    HoverInterruptButton,
    UnhoverInterruptButton,
    OpenBrowser { dir: String, file: Option<String> },
    Error(String),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Focus,

    // Kill: The caller wants to kill this tab. This tab will show a popup "quit session?".
    // KillBeProcess: If the user clicked "yes" for "quit session?", this message is produced.
    //                It'll kill the backend process and produce `IcedMessage::Dead`.
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    KillBeProcess,
    Dead,
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { IcedMessage::BackPopup }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
}

impl ChatMessage for IcedMessage {
    fn hover_button() -> IcedMessage {
        IcedMessage::HoverInterruptButton
    }

    fn unhover_button() -> IcedMessage {
        IcedMessage::UnhoverInterruptButton
    }

    fn edit(action: TextEditorAction) -> IcedMessage {
        IcedMessage::EditInterruptText(action)
    }

    fn enter() -> IcedMessage {
        IcedMessage::InterruptNeukgu
    }
}

#[derive(Clone, Debug)]
pub enum Popup {
    GetApiKeys,
    Turn(usize, TurnId),
    Logs,
    Log((String, LogId)),
    Summaries,
    Summary(SessionSummary),
    FileChanges(Vec<FileChange>),
    Help,
    Image(ImageId),
    Diff,
    TokenUsage,
    Prompt,
    Instruction,
    Config,
    Reset,
    Find { re: Option<String>, error: Option<String> },
    AskRollBack { id: TurnId, title: String },
    AskQuit,
}

impl Popup {
    pub fn has_short_text_input(&self) -> bool {
        matches!(self, Popup::Find { .. })
    }

    pub fn has_long_text_input(&self) -> bool {
        matches!(self, Popup::Reset)
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick { frame, force_update } => {
            if frame % 4 == 0 || force_update {
                context.fe_context.end_frame(
                    context.pause.take(),
                    context.question_from_user.take(),
                    context.user_response.take(),
                    context.has_to_update_config,
                )?;
                context.has_to_update_config = false;
                context.update_find_result();

                // These are too expensive.
                if frame % 16 == 0 || force_update {
                    if context.loaded_logs.is_some() {
                        let logs = load_logs_tail(&context.log_dir)?;
                        context.copy_buffer = Some(logs.join("\n"));
                        context.loaded_logs = Some(logs);
                    }

                    if let Some(Popup::TokenUsage) = context.curr_popup {
                        let token_usage = context.fe_context.get_token_usage()?;
                        context.set_long_text_editor_content(token_usage.to_string());
                        context.copy_buffer = Some(token_usage.to_string());
                    }

                    if let Some(Popup::FileChanges(_)) = context.curr_popup {
                        context.update_file_changes()?;
                    }
                }

                let llm_request = context.fe_context.get_llm_request()?;

                if let Some((id, _)) = &llm_request {
                    if !context.processed_llm_requests.contains(id) {
                        if context.llm_request.is_none() {
                            context.close_popup();
                            context.user_response_timeout_counter = Instant::now();
                            context.llm_request = llm_request;
                            return Ok(focus(context.long_text_editor_id.clone()));
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
            }

            return Ok(is_focused(context.interrupt_text_editor_id.clone()).map(|is_focused| IcedMessage::IsInterruptTextEditorFocused(is_focused)));
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Backspace), false, false, false) => {
                if context.curr_popup.is_some() && context.prev_popup.is_some() {
                    return Ok(Task::done(IcedMessage::BackPopup));
                }
            },
            (Key::Named(NamedKey::Escape), false, false, false) => {
                if context.can_close_popup() {
                    return Ok(Task::done(IcedMessage::ClosePopup));
                }

                else if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }));
                }
            },
            (Key::Named(NamedKey::ArrowUp), ctrl, false, false) => {
                if context.can_click_turn_entry() {
                    let scroll_index = context.select_turn(if ctrl { -10 } else { -1 });
                    return Ok(scroll_to(context.turn_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }

                else if ctrl && context.curr_popup.is_some() {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 0.0 }));
                }
            },
            (Key::Named(NamedKey::ArrowDown), ctrl, false, false) => {
                if context.can_click_turn_entry() {
                    let scroll_index = context.select_turn(if ctrl { 10 } else { 1 });
                    return Ok(scroll_to(context.turn_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }

                else if ctrl && context.curr_popup.is_some() {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }));
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
                if context.can_click_turn_entry() && let Some(i) = context.selected_turn {
                    match context.fe_context.history.get(i) {
                        Some(turn) => {
                            return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Turn(i, turn.id.clone()), prev: None }));
                        },
                        None => {},
                    }
                }
            },
            (Key::Named(NamedKey::Tab), true, false, false) => {
                if context.can_click_turn_entry() {
                    context.is_interrupt_text_editor_focused = true;
                    return Ok(focus(context.interrupt_text_editor_id.clone()));
                }

                // This doesn't work because when the editor is focused, the key presses are not passed to this branch.
                // else if context.is_interrupt_text_editor_focused {
                //     return Ok(Task::done(IcedMessage::InterruptNeukgu));
                // }
            },
            (Key::Named(NamedKey::Space), false, false, false) => {
                if context.can_click_turn_entry() {
                    if context.is_paused {
                        return Ok(Task::done(IcedMessage::ResumeNeukgu));
                    } else {
                        return Ok(Task::done(IcedMessage::PauseNeukgu));
                    }
                }
            },
            (Key::Character("c"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Config, prev: None }));
                }

                else if context.copy_buffer.is_some() {
                    return Ok(Task::done(IcedMessage::CopyPopupContent));
                }
            },
            (Key::Character("d"), true, false, false) => {
                if let Some(Popup::Turn(_, _)) = &context.curr_popup && context.text_diff.is_some() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Diff, prev: context.curr_popup.clone() }));
                }
            },
            (Key::Character("f"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Find { re: context.find_pattern.as_ref().map(|(pattern, _)| pattern.to_string()), error: None }, prev: None }));
                }
            },
            (Key::Character("g"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::FileChanges(vec![]), prev: None }));
                }
            },
            (Key::Character("h"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Help, prev: None }));
                }
            },
            (Key::Character("l"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }));
                }
            },
            (Key::Character("o"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenBrowser { dir: context.fe_context.working_dir.to_string(), file: None }));
                }

                else if let Some(Popup::Turn(_, _)) = &context.curr_popup && let Some((dir, file)) = &context.turn_result_path {
                    return Ok(Task::done(IcedMessage::OpenBrowser { dir: dir.to_string(), file: file.clone() }));
                }
            },
            (Key::Character("r"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Reset, prev: None }));
                }
            },
            (Key::Character("q"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }));
                }
            },
            (Key::Character("s"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Summaries, prev: None }));
                }

                else if let Some((_, turn)) = &context.loaded_turn && let Some(log_id) = &turn.api_log.response_body {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Log((turn.id.0.to_string(), log_id.clone())), prev: context.curr_popup.clone() }));
                }
            },
            (Key::Character("u"), true, false, false) => {
                if context.can_click_turn_entry() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }));
                }
            },
            (Key::Character("v"), true, false, false) => {
                if context.can_click_turn_entry() {
                    if let Some(i) = context.selected_turn && let Some(turn) = &context.fe_context.history.get(i) {
                        return Ok(Task::done(IcedMessage::ToggleTurnVisibility(turn.id.clone())));
                    }
                }
            },
            (Key::Character("y"), true, false, false) => {
                if let Some(Popup::AskRollBack { id, .. }) = &context.curr_popup && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::RollBackNeukgu(id.clone())));
                }

                if let Some(Popup::AskQuit) = context.curr_popup && context.llm_request.is_none() {
                    return Ok(Task::done(IcedMessage::KillBeProcess));
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
        IcedMessage::GetApiKeys(m) => match m {
            GetApiKeysMessage::Enter => {
                if let Some((env_var, _)) = context.get_api_keys_context.missing_api_keys.get(0) {
                    context.api_keys.insert(env_var.to_string(), context.get_api_keys_context.key1.to_string());
                }

                if let Some((env_var, _)) = context.get_api_keys_context.missing_api_keys.get(1) {
                    context.api_keys.insert(env_var.to_string(), context.get_api_keys_context.key2.to_string());
                }

                if let Some((env_var, _)) = context.get_api_keys_context.missing_api_keys.get(2) {
                    context.api_keys.insert(env_var.to_string(), context.get_api_keys_context.key3.to_string());
                }

                if let Some((env_var, _)) = context.get_api_keys_context.missing_api_keys.get(3) {
                    context.api_keys.insert(env_var.to_string(), context.get_api_keys_context.key4.to_string());
                }

                context.spawn_be_process()?;
                context.close_popup();
            },
            m => {
                return Ok(api_key::update(&mut context.get_api_keys_context, m).map(IcedMessage::GetApiKeys));
            },
        },
        IcedMessage::TurnViewScrolled(o) => {
            context.turn_view_scrolled = o;
        },
        IcedMessage::HoverOnTurn(id) => {
            context.hovered_turn = id;
        },
        IcedMessage::ExpandFileChange(path) => {
            if let Some(Popup::FileChanges(changes)) = &mut context.curr_popup {
                for change in changes.iter_mut() {
                    if change.path == path {
                        change.expanded = !change.expanded;
                        break;
                    }
                }
            }
        },
        IcedMessage::ExpandAllFileChanges { log, expand } => {
            if let Some(Popup::FileChanges(changes)) = &mut context.curr_popup {
                for change in changes.iter_mut() {
                    if change.path.starts_with("logs/") == log {
                        change.expanded = expand;
                    }
                }
            }
        },
        IcedMessage::OpenPopup { curr, prev } => {
            let mut tasks: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled),
            ];

            if curr.has_long_text_input() {
                tasks.push(focus(context.long_text_editor_id.clone()));
            }

            else if curr.has_short_text_input() {
                tasks.push(focus(context.short_text_editor_id.clone()));
            }

            if let Popup::Logs = &curr {
                tasks.push(snap_to(context.popup_scroll_id.clone(), RelativeOffset::END));
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
            if let Some(Popup::Find { .. }) = &context.curr_popup {
                context.find_pattern = None;
                context.update_find_result();
            }

            context.close_popup();
            return Ok(scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled));
        },
        IcedMessage::CopyPopupContent => {
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
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
        },
        IcedMessage::PauseNeukgu => {
            context.pause = Some(true);
            context.fe_context.interrupt_be()?;
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
        },
        IcedMessage::ResumeNeukgu => {
            context.pause = Some(false);
        },
        IcedMessage::RespawnBeProcess => {
            context.spawn_be_process()?;
        },
        IcedMessage::InterruptNeukgu => {
            let question = context.interrupt_text_editor_content.text();
            context.is_interrupt_button_hovered = false;

            if !question.is_empty() {
                context.question_from_user = Some((rand::random::<u64>(), question));
                context.set_interrupt_text_editor_content(String::new());
                context.fe_context.interrupt_be()?;
                return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
            }
        },
        IcedMessage::ResetNeukgu => {
            context.kill_be_process()?;
            reset_working_dir(context.long_text_editor_content.text(), &context.fe_context.working_dir)?;
            context.spawn_be_process()?;
            context.fe_context = FeContext::load(&context.fe_context.working_dir)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
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
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
        },
        IcedMessage::SetTmpConfig(c) => {
            set_project_config(&mut context.tmp_config, c);
        },
        IcedMessage::ApplyTmpConfig => {
            context.close_popup();

            if context.fe_context.config != context.tmp_config {
                context.fe_context.config = context.tmp_config.clone();
                context.fe_context.interrupt_be()?;
                context.has_to_update_config = true;
                return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
            }
        },
        IcedMessage::Find => {
            let pattern = context.short_text_editor_content.to_string();

            if pattern.is_empty() {
                context.find_pattern = None;
            }

            else {
                match Regex::new(&pattern) {
                    Ok(re) => {
                        context.find_pattern = Some((pattern, re));
                    },
                    Err(e) => {
                        return Ok(Task::done(IcedMessage::OpenPopup {
                            curr: Popup::Find { re: Some(pattern), error: Some(format!("{e:?}")) },
                            prev: None,
                        }));
                    },
                }
            }

            context.close_popup();
            context.update_find_result();
        },
        IcedMessage::AnswerLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Answer(context.long_text_editor_content.text())));
            context.long_text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::DismissLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Reject));
            context.long_text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::EditShortText(s) => {
            context.short_text_editor_content = s;
        },
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::EditInterruptText(a) => {
            context.interrupt_text_editor_content.perform(a);
        },
        IcedMessage::IsInterruptTextEditorFocused(f) => {
            context.is_interrupt_text_editor_focused = f;
        },
        IcedMessage::HoverInterruptButton => {
            context.is_interrupt_button_hovered = true;

            // unfocus the text editor
            return Ok(focus(context.turn_view_id.clone()));
        },
        IcedMessage::UnhoverInterruptButton => {
            context.is_interrupt_button_hovered = false;
        },
        IcedMessage::OpenBrowser { .. } => unreachable!(),
        IcedMessage::Error(_) => unreachable!(),
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(_) => todo!(),
        IcedMessage::Focus => {
            return Ok(scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled));
        },
        IcedMessage::Kill => {
            return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }));
        },
        IcedMessage::KillBeProcess => {
            context.kill_be_process()?;
            return Ok(Task::done(IcedMessage::Dead));
        },
        IcedMessage::Dead => unreachable!(),
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

    // Without this, interrupt_text_editor will hide the error message
    turns.push(text!("").width(context.window_size.width).height(context.window_size.height * 0.3).into());

    let turns_stretched = Column::from_vec(turns)
        .padding(context.zoom * 8.0)
        .spacing(context.zoom * 8.0);

    let mut turns_scrollable = Scrollable::new(turns_stretched).id(context.turn_view_id.clone());

    if context.can_click_turn_entry() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::TurnViewScrolled(v.absolute_offset()));
    }

    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));
    let mut full_view = vec![
        Container::new(text!("{}", context.fe_context.top_bar()).size(context.zoom * 14.0)).padding(context.zoom * 8.0).into(),
        render_buttons(context),
    ];

    if let Some((pattern, _)) = &context.find_pattern {
        let matches = context.fe_context.iter_previews().iter().filter(
            |preview| context.find_result.contains_key(&preview.preview_title_truncated)
        ).count();
        full_view.push(text!(
            "find: {pattern:?}, found {matches} result{}",
            if matches == 1 { "" } else { "s" },
        ).size(context.zoom * 14.0).into());
    }

    full_view.push(turns_colored.into());

    // Without this, interrupt_text_editor won't be seen if there are too small turns
    full_view.push(Space::new().width(context.window_size.width).height(context.window_size.height).into());

    let full_view = Column::from_vec(full_view);
    let full_view_with_text_input = Stack::from_vec(vec![
        full_view.into(),
        chat_ui(
            context.is_interrupt_text_editor_focused,
            context.curr_popup.is_none() && context.llm_request.is_none(),
            context.is_interrupt_button_hovered,
            context.interrupt_text_editor_id.clone(),
            &context.interrupt_text_editor_content,
            "Interrupt",
            Space::new().into(),
            0.0,
            context.window_size,
            context.zoom,
        ),
    ]);

    let mut full_view_stacked: Element<IcedMessage> = full_view_with_text_input.into();

    if let Some(Popup::GetApiKeys) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            get_api_keys_popup(
                &context.get_api_keys_context,
                context,
                context.zoom,
            ).map(|m| IcedMessage::GetApiKeys(m)),
        ]).into();
    }

    else if context.llm_request.is_some() {
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
        let view = render_logs(logs, context, context.popup_scroll_id.clone(), context.zoom);
        full_view_stacked = Stack::from_vec(vec![full_view_stacked, view]).into();
    }

    else if let Some(loaded_image) = context.loaded_image {
        let image_view: Element<_> = into_popup(
            ImageViewer::new(ImageHandle::from_path(loaded_image.path(&context.fe_context.working_dir).unwrap())).content_fit(ContentFit::Contain).into(),
            context,
        ).into();

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            image_view,
        ]).into();
    }

    else if let Some(Popup::Summaries) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_summaries(&context.fe_context.summaries, context),
        ]).into();
    }

    else if let Some(Popup::Summary(summary)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_summary(summary, context),
        ]).into();
    }

    else if let Some(Popup::Diff) = context.curr_popup {
        let diff_view = Scrollable::new(render_udiff(context.text_diff.as_ref().unwrap(), context.window_size.width, context)).id(context.popup_scroll_id.clone());
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(diff_view.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::FileChanges(changes)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(render_file_changes(changes, context), context).into(),
        ]).into();
    }

    else if let Some(Popup::Config) = context.curr_popup {
        let config_popup = Scrollable::new(
            Column::from_vec(vec![
                text!("Config").size(context.zoom * 18.0).into(),
                config_ui(&context.tmp_config, context.zoom).map(|m| IcedMessage::SetTmpConfig(m)).into(),
                button("Apply", IcedMessage::ApplyTmpConfig, green(), context.zoom).into(),
            ])
                .align_x(Horizontal::Center)
                .width(Length::Fill)
                .spacing(context.zoom * 12.0)
        ).id(context.popup_scroll_id.clone());
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(config_popup.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Reset) = context.curr_popup {
        let text_editor = TextEditor::new(&context.long_text_editor_content)
            .size(context.zoom * 14.0)
            .placeholder("What do you want neukgu to do?")
            .min_height(400)
            .id(context.long_text_editor_id.clone())
            .on_action(|action| IcedMessage::EditLongText(action));
        let reset_edit = Column::from_vec(vec![
            text!("New instruction").size(context.zoom * 14.0).into(),
            text_editor.into(),
            button("Reset", IcedMessage::ResetNeukgu, green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);
        let reset_edit = Scrollable::new(reset_edit);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(reset_edit.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Find { error, .. }) = &context.curr_popup {
        let text_editor = TextInput::new("regex", &context.short_text_editor_content)
            .size(context.zoom * 14.0)
            .id(context.short_text_editor_id.clone())
            .on_input(|input| IcedMessage::EditShortText(input))
            .on_submit(IcedMessage::Find);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(
                Column::from_vec(vec![
                    text_editor.into(),
                    if let Some(error) = error {
                        text!("{error}").size(context.zoom * 14.0).color(red()).into()
                    } else {
                        Space::new().into()
                    },
                    button("Find", IcedMessage::Find, green(), context.zoom).padding(context.zoom * 20.0).into(),
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .into(),
                context,
            ).into(),
        ]).into();
    }

    else if let Some(Popup::AskRollBack { id, title }) = &context.curr_popup {
        let q = Column::from_vec(vec![
            text!("Roll back to {title}?").size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::RollBackNeukgu(id.clone()), green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(q.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::AskQuit) = context.curr_popup {
        let q = Column::from_vec(vec![
            text!("Quit session?").size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::KillBeProcess, green(), context.zoom).padding(context.zoom * 20.0).into(),
        ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill);
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(q.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Log(_) | Popup::Help | Popup::TokenUsage | Popup::Prompt | Popup::Instruction) = &context.curr_popup {
        let title = text!("{}", context.popup_title.clone().unwrap_or(String::new()))
            .width(context.window_size.width)
            .size(context.zoom * 18.0);
        let text_editor = TextEditor::new(&context.long_text_editor_content).size(context.zoom * 14.0).highlight(
            &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
            iced::highlighter::Theme::SolarizedDark,
        );

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(
                Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    text_editor.into(),
                ]).spacing(context.zoom * 8.0))
                    .width(Length::Fill)
                    .id(context.popup_scroll_id.clone())
                    .into(),
                context,
            ),
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let mut buttons_row1: Vec<Button<IcedMessage>> = if context.be_process.is_none() {
        vec![button("Respawn", IcedMessage::RespawnBeProcess, blue(), context.zoom)]
    } else if context.is_paused {
        vec![button("Resume", IcedMessage::ResumeNeukgu, blue(), context.zoom)]
    } else {
        vec![button("Pause", IcedMessage::PauseNeukgu, blue(), context.zoom)]
    };
    let mut buttons_row2: Vec<Button<IcedMessage>> = vec![];

    buttons_row1.push(button("(Q)uit", IcedMessage::OpenPopup { curr: Popup::AskQuit, prev: None }, red(), context.zoom));
    buttons_row1.push(button("See (l)ogs", IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }, yellow(), context.zoom));
    buttons_row1.push(button("See (s)ummaries", IcedMessage::OpenPopup { curr: Popup::Summaries, prev: None }, yellow(), context.zoom));
    buttons_row1.push(button("Token (u)sage", IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }, yellow(), context.zoom));
    buttons_row1.push(button("File Chan(g)es", IcedMessage::OpenPopup { curr: Popup::FileChanges(vec![]), prev: None }, yellow(), context.zoom));
    buttons_row1.push(button("(H)elp", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink(), context.zoom));

    buttons_row2.push(button("Prompt", IcedMessage::OpenPopup { curr: Popup::Prompt, prev: None }, yellow(), context.zoom));
    buttons_row2.push(button("Instruction", IcedMessage::OpenPopup { curr: Popup::Instruction, prev: None }, yellow(), context.zoom));
    buttons_row2.push(button("(C)onfig", IcedMessage::OpenPopup { curr: Popup::Config, prev: None }, blue(), context.zoom));
    buttons_row2.push(button("Br(o)wser", IcedMessage::OpenBrowser { dir: context.fe_context.working_dir.to_string(), file: None }, skyblue(), context.zoom));
    buttons_row2.push(button("(F)ind", IcedMessage::OpenPopup { curr: Popup::Find { re: context.find_pattern.as_ref().map(|(pattern, _)| pattern.to_string()), error: None }, prev: None }, blue(), context.zoom));
    buttons_row2.push(button("(R)eset", IcedMessage::OpenPopup { curr: Popup::Reset, prev: None }, blue(), context.zoom));

    let buttons_row1 = if !context.can_click_turn_entry() {
        buttons_row1.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons_row1.into_iter().map(|button| button.into()).collect()
    };
    let buttons_row2 = if !context.can_click_turn_entry() {
        buttons_row2.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons_row2.into_iter().map(|button| button.into()).collect()
    };

    Column::from_vec(vec![
        Row::from_vec(buttons_row1).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
        Row::from_vec(buttons_row2).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
    ]).into()
}

fn render_turn_preview<'t, 'c, 'm>(index: usize, p: &'t TurnPreview, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let roll_back = {
        let (color, text, enabled) = if context.fe_context.snapshots.contains(&p.id) {
            (red(), "R", context.can_click_turn_entry())
        } else {
            (gray(0.2), " ", false)
        };

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

        if context.can_click_turn_entry() {
            button(text, IcedMessage::ToggleTurnVisibility(p.id.clone()), color, context.zoom)
        } else {
            disabled_button(text, color, context.zoom)
        }
    };

    let turn_result: Element<IcedMessage> = match p.result {
        TurnResultSummary::ParseError => text!(" (parse-error)").size(context.zoom * 14.0).color(red()),
        TurnResultSummary::ToolCallError => text!(" (tool-call-error)").size(context.zoom * 14.0).color(yellow()),
        TurnResultSummary::ToolCallSuccess => text!("").size(context.zoom * 14.0),
    }.into();

    let preview_title: Element<IcedMessage> = if let Some((start, end)) = context.find_result.get(&p.preview_title_truncated) {
        let (pre, m, post) = (
            p.preview_title_truncated.get(0..*start).unwrap(),
            p.preview_title_truncated.get(*start..*end).unwrap(),
            p.preview_title_truncated.get(*end..).unwrap(),
        );
        Row::from_vec(vec![
            text!("{pre}").size(context.zoom * 14.0).into(),
            Container::new(text!("{m}").color(black()).size(context.zoom * 14.0)).style(|_| set_bg(white())).into(),
            text!("{post}").size(context.zoom * 14.0).into(),
        ]).into()
    } else {
        text!("{}", p.preview_title_truncated).size(context.zoom * 14.0).into()
    };

    let turn_row = Row::from_vec(vec![
        text!("{index:>3}. ").size(context.zoom * 14.0).into(),
        Column::from_vec(vec![
            text!("[{}]", p.timestamp).size(context.zoom * 14.0).into(),
            text!("({})", prettify_timestamp(p.timestamp_millis)).size(context.zoom * 14.0).into(),
        ]).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                preview_title,
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

    let with_mouse_area: Element<IcedMessage> = if context.can_click_turn_entry() {
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

fn render_turn<'t, 'c>(index: usize, turn: &'t Turn, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut turn_content = vec![
        text!("# {index}. {}", turn.preview().preview_title).size(context.zoom * 14.0).into(),
        text!("<|LLM|>").size(context.zoom * 14.0).into(),
        Container::new(
            render_llm_tokens(vec![LLMToken::String(turn.render_llm_response(true))], &context.fe_context.working_dir, context.zoom, context)
        ).padding(context.zoom * 8.0).style(|_| set_bg(gray(0.3))).into(),
        text!("<|result|>").size(context.zoom * 14.0).into(),
        Container::new(
            render_llm_tokens(turn.turn_result.to_llm_tokens(&context.fe_context.config), &context.fe_context.working_dir, context.zoom, context)
        ).padding(context.zoom * 8.0).style(|_| set_bg(gray(0.3))).into(),
    ];
    turn_content.push(text!("{}", turn.introduce_agents()).size(context.zoom * 14.0).into());

    let mut buttons = vec![];

    if context.text_diff.is_some() {
        buttons.push(button(
            "(D)iff",
            IcedMessage::OpenPopup { curr: Popup::Diff, prev: context.curr_popup.clone() },
            yellow(),
            context.zoom,
        ).into());
    }

    if let Some((dir, file)) = &context.turn_result_path {
        buttons.push(button(
            "(O)pen in browser",
            IcedMessage::OpenBrowser { dir: dir.to_string(), file: file.clone() },
            skyblue(),
            context.zoom,
        ).into());
    }

    if let Some(log_id) = &turn.api_log.response_body {
        buttons.push(button(
            "Raw LLM re(s)ponse",
            IcedMessage::OpenPopup { curr: Popup::Log((turn.id.0.to_string(), log_id.clone())), prev: context.curr_popup.clone() },
            yellow(),
            context.zoom,
        ).into());
    }

    if !buttons.is_empty() {
        turn_content.push(Row::from_vec(buttons).spacing(context.zoom * 8.0).into());
    }

    let turn_content = Scrollable::new(
        Column::from_vec(turn_content).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).width(Length::Fill)
    ).id(context.popup_scroll_id.clone()).width(Length::Fill);
    into_popup(turn_content.into(), context)
}

pub trait ImagePopup {
    type Message;
    fn open_image_popup(&self, id: ImageId) -> Self::Message;
}

pub fn render_llm_tokens<'c, Context: ImagePopup<Message=Message>, Message: Clone + 'c>(
    llm_tokens: Vec<LLMToken>,
    working_dir: &str,
    zoom: f32,
    context: &'c Context,
) -> Column<'c, Message> {
    Column::from_vec(llm_tokens.iter().map(
        |token| match token {
            LLMToken::String(s) => text!("{s}").size(zoom * 14.0).width(Length::Fill).into(),
            LLMToken::Image(id) => MouseArea::new(
                Image::new(ImageHandle::from_path(id.path(working_dir).unwrap()))
                    .width(Length::Fixed(zoom * 480.0))
                    .height(Length::Fixed(zoom * 480.0))
                    .content_fit(ContentFit::Contain),
            ).on_press(context.open_image_popup(*id)).into(),
        }
    ).collect()).width(Length::Fill)
}

fn render_ask_to_user_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let elapsed_secs = Instant::now().duration_since(context.user_response_timeout_counter.clone()).as_secs();

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                text!("{}", context.llm_request.as_ref().unwrap().1).size(context.zoom * 14.0).into(),
                TextEditor::new(&context.long_text_editor_content)
                    .id(context.long_text_editor_id.clone())
                    .placeholder("Answer neukgu's question")
                    .size(context.zoom * 14.0)
                    .width(context.window_size.width - context.zoom * 128.0)
                    .on_action(|action| IcedMessage::EditLongText(action))
                    .key_binding(|key_press| {
                        let KeyPress { key, modifiers, .. } = &key_press;

                        match (key.as_ref(), modifiers.control()) {
                            (Key::Named(NamedKey::Enter), true) => Some(Binding::Sequence(vec![Binding::Unfocus, Binding::Custom(IcedMessage::AnswerLLMRequest)])),
                            _ => Binding::from_key_press(key_press),
                        }
                    })
                    .into(),
                Row::from_vec(vec![
                    button("Answer", IcedMessage::AnswerLLMRequest, green(), context.zoom).into(),
                    button("Dismiss", IcedMessage::DismissLLMRequest, red(), context.zoom).into(),
                    text!("{}", context.fe_context.config.user_response_timeout.max(elapsed_secs) - elapsed_secs).size(context.zoom * 14.0).into(),
                ]).spacing(context.zoom * 20.0).into(),
            ])
                .padding(context.zoom * 20.0)
                .spacing(context.zoom * 20.0),
        )
            .id(context.popup_scroll_id.clone())
            .into(),
        context,
    )
}

fn render_summaries<'s, 'c>(summaries: &'s [SessionSummary], context: &'c IcedContext) -> Element<'c, IcedMessage> {
    if summaries.is_empty() {
        return into_popup(text!("(There are no summaries yet.)").size(context.zoom * 14.0).into(), context);
    }

    into_popup(
        Scrollable::new(
            Column::from_vec(summaries.iter().map(
                |summary| {
                    let mut truncated = false;
                    let summary_preview: String = {
                        let mut chars = vec![];
                        let mut line_count = 0;

                        for ch in summary.summary.chars() {
                            chars.push(ch);

                            if ch == '\n' {
                                line_count += 1;
                            }

                            if line_count == 4 || chars.len() > 256 {
                                truncated = true;
                                break;
                            }
                        }

                        chars.into_iter().collect()
                    };
                    let summary_preview = Container::new(
                        text!("{summary_preview}{}", if truncated { "..." } else { "" }).size(context.zoom * 14.0)
                    ).style(|_| Style {
                        background: Some(Background::Color(gray(0.15))),
                        border: Border {
                            color: white(),
                            width: context.zoom * 2.0,
                            radius: Radius::new(context.zoom * 8.0),
                        },
                        ..Style::default()
                    }).width(context.window_size.width).padding(context.zoom * 8.0);

                    Container::new(Column::from_vec(vec![
                        Row::from_vec(vec![
                            text!("{}", summary.title).size(context.zoom * 18.0).into(),
                            text!("({})", prettify_timestamp(summary.timestamp_millis)).size(context.zoom * 12.0).into(),
                        ]).spacing(context.zoom * 4.0).align_y(Vertical::Bottom).into(),
                        summary_preview.into(),
                        if truncated {
                            button("more", IcedMessage::OpenPopup { curr: Popup::Summary(summary.clone()), prev: Some(Popup::Summaries) }, yellow(), context.zoom).into()
                        } else {
                            Space::new().into()
                        },
                    ]).spacing(context.zoom * 8.0)).style(|_| Style {
                        background: Some(Background::Color(gray(0.15))),
                        border: Border {
                            color: white(),
                            width: 0.0,
                            radius: Radius::new(context.zoom * 8.0),
                        },
                        ..Style::default()
                    }).padding(context.zoom * 8.0).into()
                }
            ).collect())
                .padding(context.zoom * 8.0)
                .spacing(context.zoom * 16.0)
        ).id(context.popup_scroll_id.clone()).into(),
        context,
    )
}

fn render_summary<'s, 'c>(summary: &'s SessionSummary, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                Row::from_vec(vec![
                    text!("{}", summary.title).size(context.zoom * 18.0).into(),
                    text!("[{}] ({})", summary.timestamp, prettify_timestamp(summary.timestamp_millis)).size(context.zoom * 12.0).into(),
                ]).spacing(context.zoom * 4.0).align_y(Vertical::Bottom).into(),
                TextEditor::new(&context.long_text_editor_content)
                    .width(context.window_size.width)
                    .size(context.zoom * 14.0)
                    .highlight("md", iced::highlighter::Theme::SolarizedDark)
                    .into(),
            ]).padding(context.zoom * 8.0).spacing(context.zoom * 20.0)
        ).id(context.popup_scroll_id.clone()).into(),
        context,
    )
}

fn render_file_changes<'ch, 'co>(changes: &'ch [FileChange], context: &'co IcedContext) -> Element<'co, IcedMessage> {
    fn render_file_change<'ch, 'co, 'ae>(change: &'ch FileChange, context: &'co IcedContext, all_expanded: &'ae mut bool) -> Element<'co, IcedMessage> {
        let (mut add, mut remove) = (0, 0);

        for line in change.udiff.lines() {
            if line.starts_with("+") {
                add += 1;
            } else if line.starts_with("-") {
                remove += 1;
            }
        }

        let view: Element<IcedMessage> = if change.expanded {
            Column::from_vec(vec![
                Row::from_vec(vec![
                    button("▼", IcedMessage::ExpandFileChange(change.path.to_string()), white(), context.zoom).into(),
                    Space::new().width(context.zoom * 8.0).into(),
                    text!("{} (", change.path).size(context.zoom * 14.0).into(),
                    text!("+{add}").size(context.zoom * 14.0).color(green()).into(),
                    text!(", ").size(context.zoom * 14.0).into(),
                    text!("-{remove}").size(context.zoom * 14.0).color(red()).into(),
                    text!(")").size(context.zoom * 14.0).into(),
                ]).align_y(Vertical::Center).into(),
                render_udiff(&change.udiff, context.window_size.width, context),
            ]).into()
        } else {
            *all_expanded = false;
            Row::from_vec(vec![
                button("▶", IcedMessage::ExpandFileChange(change.path.to_string()), white(), context.zoom).into(),
                Space::new().width(context.zoom * 8.0).into(),
                text!("{} (", change.path).size(context.zoom * 14.0).into(),
                text!("+{add}").size(context.zoom * 14.0).color(green()).into(),
                text!(", ").size(context.zoom * 14.0).into(),
                text!("-{remove}").size(context.zoom * 14.0).color(red()).into(),
                text!(")").size(context.zoom * 14.0).into(),
            ]).align_y(Vertical::Center).into()
        };

        Container::new(view)
            .width(context.window_size.width)
            .padding(context.zoom * 8.0)
            .style(|_| Style {
                background: Some(Background::Color(gray(0.25))),
                border: Border {
                    color: white(),
                    width: 0.0,
                    radius: Radius::new(context.zoom * 8.0),
                },
                ..Style::default()
            })
            .into()
        }

    let mut all_files_expanded = true;
    let file_changes: Vec<&FileChange> = changes.iter().filter(|change| !change.path.starts_with("logs/")).collect();
    let mut all_logs_expanded = true;
    let log_changes: Vec<&FileChange> = changes.iter().filter(|change| change.path.starts_with("logs/")).collect();
    let mut changes = vec![];
    changes.extend(file_changes.iter().map(|change| render_file_change(change, context, &mut all_files_expanded)));
    let insert_log_title_at = changes.len();
    changes.extend(log_changes.iter().map(|change| render_file_change(change, context, &mut all_logs_expanded)));
    changes.insert(0, Row::from_vec(vec![
        text!("File Changes").size(context.zoom * 18.0).into(),
        if all_files_expanded {
            button("Collapse all", IcedMessage::ExpandAllFileChanges { log: false, expand: false }, white(), context.zoom).into()
        } else {
            button("Expand all", IcedMessage::ExpandAllFileChanges { log: false, expand: true }, white(), context.zoom).into()
        },
    ]).align_y(Vertical::Center).spacing(context.zoom * 12.0).into());
    changes.insert(insert_log_title_at + 1, Row::from_vec(vec![
        text!("Log Changes").size(context.zoom * 18.0).into(),
        if all_logs_expanded {
            button("Collapse all", IcedMessage::ExpandAllFileChanges { log: true, expand: false }, white(), context.zoom).into()
        } else {
            button("Expand all", IcedMessage::ExpandAllFileChanges { log: true, expand: true }, white(), context.zoom).into()
        },
    ]).align_y(Vertical::Center).spacing(context.zoom * 12.0).into());

    Container::new(
        Scrollable::new(
            Column::from_vec(changes)
                .width(context.window_size.width)
                .spacing(context.zoom * 8.0)
        ).id(context.popup_scroll_id.clone())
    ).padding(context.zoom * 12.0).into()
}

fn get_missing_api_keys(api_keys: &HashMap<String, String>, config: &Config) -> Vec<(String, Vec<String>)> {  // Vec<(env_var, Vec<agent_name>)>
    let mut missing_api_keys: HashMap<String, Vec<String>> = HashMap::new();

    for (env_var, agent_name) in config.agents.iter_with_name().filter_map(|(model, name)| if model == Model::Disabled { None } else { Some((model.api_key_env_var(), name.to_string())) }) {
        if std::env::var(env_var).is_err() && !api_keys.contains_key(env_var) {
            match missing_api_keys.entry(env_var.to_string()) {
                Entry::Occupied(mut e) => {
                    e.get_mut().push(agent_name);
                },
                Entry::Vacant(e) => {
                    e.insert(vec![agent_name]);
                },
            }
        }
    }

    let mut missing_api_keys: Vec<(String, Vec<String>)> = missing_api_keys.into_iter().collect();
    missing_api_keys.sort_by_key(|(key, _)| key.to_string());
    missing_api_keys
}
