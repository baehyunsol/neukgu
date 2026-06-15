use super::{
    TEXT_EDITOR_CONTENT_LIMIT,
    black,
    blue,
    button,
    disabled_button,
    gray,
    pink,
    purple,
    red,
    set_bg,
    set_round_bg,
    skyblue,
    white,
    yellow,
};
use super::api_key::{
    self,
    IcedContext as GetApiKeysContext,
    IcedMessage as GetApiKeysMessage,
    get_api_keys_popup,
};
use super::config::{SetChatConfig, chat_config_ui1, set_chat_config};
use super::logs::{LogsContext, render_logs, render_token_usage};
use super::popup::{PopupContext, PopupMessage, into_popup};
use super::scratch_pad::Content as ScratchPadContent;
use super::worker::{
    Job,
    JobId,
    JobKind,
    JobResult,
    JobResultKind,
};
use super::working_dir::{
    ImagePopup,
    render_llm_tokens,
};
use chrono::Local;
use crate::{
    Chat,
    ChatId,
    ChatTurn,
    ChatTurnId,
    Error,
    ImageId,
    LogId,
    LLMToken,
    Model,
    TokenUsage,
    WebSearchResult,
    get_global_index_dir,
    load_json,
    load_log,
    load_logs_tail,
    prettify_timestamp,
    stringify_llm_tokens,
};
use iced::{Color, Element, Length, Padding, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, Container, Id, MouseArea, Row, Scrollable, Space, Stack, TextInput, text};
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
use std::collections::hash_map::{Entry, HashMap};
use std::sync::{Arc, LazyLock};

const HELP_MESSAGE: &str = "(TODO: write help message)";

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub api_keys: HashMap<String, String>,
    pub get_api_keys_context: GetApiKeysContext,
    pub chat: Chat,
    pub turns: HashMap<ChatTurnId, ChatTurn>,
    pub ai_code_blocks: HashMap<ChatTurnId, Vec<(String, Option<String>)>>,
    pub user_code_blocks: HashMap<ChatTurnId, Vec<(String, Option<String>)>>,

    // Whenever `bg_job` is set, this timestamp_millis is also set.
    pub bg_at: Option<i64>,

    // If it's `Some(_)`, that means the LLM is working in background,
    // so the user cannot send a new message.
    pub bg_job: Option<JobId>,

    // An error from the last bg_job.
    pub bg_error: Option<String>,

    pub curr_processing_tokens: Option<Vec<LLMToken>>,
    pub window_size: Size,
    pub global_index_dir: String,
    pub working_dir: String,
    pub log_dir: String,
    pub chat_view_id: Id,
    pub popup_scroll_id: Id,
    pub chat_input_id: Id,
    pub short_text_editor_id: Id,
    pub chat_view_scrolled: AbsoluteOffset,
    pub find_pattern: Option<(String, Regex)>,
    pub find_result: HashMap<ChatTurnId, usize>,
    pub loaded_logs: Option<Vec<String>>,
    pub loaded_token_usage: Option<TokenUsage>,
    pub loaded_image: Option<ImageId>,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub short_text_editor_content: String,
    pub long_text_editor_content: TextEditorContent,
    pub chat_input_content: TextEditorContent,
    pub syntax_highlight: Option<String>,
    pub popup_title: Option<String>,
    pub is_chat_input_focused: bool,
    pub is_chat_button_hovered: bool,
    pub zoom: f32,
}

impl IcedContext {
    pub fn new(chat_id: ChatId, api_keys: HashMap<String, String>, window_size: Size) -> Result<IcedContext, Error> {
        let global_index_dir = get_global_index_dir()?;
        let working_dir = join3(&global_index_dir, "chats", &format!("{:016x}", chat_id.0))?;
        let chat = Chat::load(chat_id, &global_index_dir)?;
        let missing_api_keys = get_missing_api_keys(&api_keys, chat.config.model);
        let mut curr_popup = None;

        if !missing_api_keys.is_empty() {
            curr_popup = Some(Popup::GetApiKeys);
        }

        let mut context = IcedContext {
            api_keys,
            get_api_keys_context: GetApiKeysContext::new(missing_api_keys),
            chat: chat.clone(),
            turns: HashMap::new(),
            ai_code_blocks: HashMap::new(),
            user_code_blocks: HashMap::new(),
            bg_at: None,
            bg_job: None,
            bg_error: None,
            curr_processing_tokens: None,
            window_size,
            global_index_dir: global_index_dir.to_string(),
            working_dir: working_dir.to_string(),
            log_dir: join3(&working_dir, ".neukgu", "logs")?,
            chat_view_id: Id::unique(),
            popup_scroll_id: Id::unique(),
            chat_input_id: Id::unique(),
            short_text_editor_id: Id::unique(),
            chat_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            find_pattern: None,
            find_result: HashMap::new(),
            loaded_logs: None,
            loaded_token_usage: None,
            loaded_image: None,
            curr_popup,
            prev_popup: None,
            copy_buffer: None,
            short_text_editor_content: String::new(),
            long_text_editor_content: TextEditorContent::new(),
            chat_input_content: TextEditorContent::new(),
            syntax_highlight: None,
            popup_title: None,
            is_chat_input_focused: false,
            is_chat_button_hovered: false,
            zoom: 1.0,
        };
        context.load_chat_turns()?;

        if let Some(unfinished_chat) = chat.unfinished_chat {
            context.fill_turn(unfinished_chat);
        }

        Ok(context)
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::GetApiKeys => unreachable!(),
            Popup::Logs => {
                let logs = load_logs_tail(&self.log_dir)?;
                self.copy_buffer = Some(logs.join("\n"));
                self.loaded_logs = Some(logs);
            },
            Popup::Log((title, id)) => {
                let (mut log, mut extension) = load_log(&id, &self.log_dir)?;
                self.copy_buffer = Some(log.to_string());

                if log.len() > TEXT_EDITOR_CONTENT_LIMIT {
                    log = String::from("The log is too long to display. Copy the log and paste it to your text editor to see the log.");
                    extension = String::from("txt");
                }

                self.set_long_text_editor_content(log.to_string());
                self.syntax_highlight = Some(extension);
                self.popup_title = Some(title);
            },
            Popup::TokenUsage => {
                self.loaded_token_usage = Some(load_json(&join(&self.log_dir, "tokens.json")?)?);
            },
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_long_text_editor_content(HELP_MESSAGE.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::ChangeTitle => {
                self.short_text_editor_content = self.chat.title.clone().unwrap_or(String::new());
            },
            Popup::Thinking(thinking) => {
                self.copy_buffer = Some(thinking.to_string());
                self.set_long_text_editor_content(thinking.to_string());
                self.popup_title = Some(String::from("Thinking"));
            },
            Popup::WebSearchResults(_) => {},
            Popup::CodeBlock { code, ext } => {
                self.copy_buffer = Some(code.to_string());
                self.set_long_text_editor_content(code.to_string());
                self.syntax_highlight = ext;
            },
            Popup::Image(_) => todo!(),
            Popup::Find { .. } => todo!(),
            Popup::FileSelector => todo!(),
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
        self.loaded_logs = None;
        self.loaded_token_usage = None;
        self.loaded_image = None;
        self.curr_popup = None;
        self.copy_buffer = None;
        self.long_text_editor_content = TextEditorContent::new();
        self.syntax_highlight = None;
        self.popup_title = None;
    }

    pub fn set_long_text_editor_content(&mut self, c: String) {
        self.long_text_editor_content.perform(TextEditorAction::SelectAll);
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn set_chat_input_content(&mut self, c: String) {
        self.chat_input_content.perform(TextEditorAction::SelectAll);
        self.chat_input_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.chat_input_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn update_find_result(&mut self) {
        // TODO
    }

    pub fn load_chat_turns(&mut self) -> Result<(), Error> {
        for turn in self.chat.turns.clone().iter() {
            if let Entry::Vacant(e) = self.turns.entry(*turn) {
                let chat_turn = ChatTurn::load(*turn, self.chat.id, &self.global_index_dir)?;
                let ai_code_blocks = extract_code_blocks(&chat_turn.assistant);
                let user_code_blocks = extract_code_blocks(&stringify_llm_tokens(&chat_turn.user));
                e.insert(chat_turn);
                self.ai_code_blocks.insert(*turn, ai_code_blocks);
                self.user_code_blocks.insert(*turn, user_code_blocks);
            }
        }

        Ok(())
    }

    pub fn fill_turn(&mut self, turn: Vec<LLMToken>) {
        for token in turn.iter() {
            match token {
                LLMToken::String(s) => {
                    self.set_chat_input_content(s.to_string());
                },
                LLMToken::Image(_) => todo!(),
            }
        }
    }

    pub fn zoom_in(&mut self) -> Task<IcedMessage> {
        if self.zoom < 2.4 {
            self.zoom += 0.1;
            Task::none()
        } else {
            Task::done(IcedMessage::Notify(String::from("Cannot zoom in anymore.")))
        }
    }

    pub fn zoom_out(&mut self) -> Task<IcedMessage> {
        if self.zoom > 0.4 {
            self.zoom -= 0.1;
            Task::none()
        } else {
            Task::done(IcedMessage::Notify(String::from("Cannot zoom out anymore.")))
        }
    }
}

impl PopupContext for IcedContext {
    fn can_close_popup(&self) -> bool { !matches!(self.curr_popup, Some(Popup::GetApiKeys) | None) }
    fn has_prev_popup(&self) -> bool { self.prev_popup.is_some() }
    fn has_something_to_copy(&self) -> bool { self.copy_buffer.is_some() }

    fn can_open_scratch_pad(&self) -> bool {
        match (&self.loaded_image, &self.copy_buffer) {
            (Some(_), _) => true,
            (_, Some(c)) if c.len() < TEXT_EDITOR_CONTENT_LIMIT => true,
            _ => false,
        }
    }

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
    ChatViewScrolled(AbsoluteOffset),
    OpenPopup { curr: Popup, prev: Option<Popup> },
    BackPopup,
    ClosePopup,
    CopyPopupContent,
    CopyString(String),
    ChangeTitle,
    SetChatConfig(SetChatConfig),
    EditShortText(String),
    EditChatInput(TextEditorAction),
    IsChatInputFocused(bool),
    HoverChatButton,
    UnhoverChatButton,
    Send,
    Error(String),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Notify(String),
    Focus,
    PrepareScratchPad,
    OpenScratchPad { title: Option<String>, content: ScratchPadContent },

    // Kill: The caller wants to kill this tab. This tab will show a popup "quit session?".
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    Dead,
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { IcedMessage::BackPopup }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
    fn open_scratch_pad() -> Self { IcedMessage::PrepareScratchPad }
}

impl ChatMessage for IcedMessage {
    fn hover_button() -> IcedMessage {
        IcedMessage::HoverChatButton
    }

    fn unhover_button() -> IcedMessage {
        IcedMessage::UnhoverChatButton
    }

    fn edit(action: TextEditorAction) -> IcedMessage {
        IcedMessage::EditChatInput(action)
    }

    fn enter() -> IcedMessage {
        IcedMessage::Send
    }
}

#[derive(Clone, Debug)]
pub enum Popup {
    GetApiKeys,
    Logs,
    Log((String, LogId)),
    TokenUsage,
    Help,
    ChangeTitle,
    Thinking(String),
    WebSearchResults(Vec<WebSearchResult>),
    CodeBlock { code: String, ext: Option<String> },
    Image(ImageId),
    Find { re: Option<String>, error: Option<String> },
    FileSelector,
}

impl Popup {
    pub fn has_short_text_input(&self) -> bool {
        match self {
            Popup::ChangeTitle | Popup::Find { .. } => true,
            _ => false,
        }
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
                context.chat = Chat::load(context.chat.id, &context.global_index_dir)?;
                context.load_chat_turns()?;
            }

            return Ok(is_focused(context.chat_input_id.clone()).map(|is_focused| IcedMessage::IsChatInputFocused(is_focused)));
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Escape), false, false, false) => {
                if context.can_close_popup() {
                    return Ok(Task::done(IcedMessage::ClosePopup));
                }
            },
            (Key::Named(NamedKey::ArrowUp), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(snap_to(context.chat_view_id.clone(), RelativeOffset { x: 0.0, y: 0.0 }));
                }

                else {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 0.0 }));
                }
            },
            (Key::Named(NamedKey::ArrowDown), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(snap_to(context.chat_view_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }));
                }

                else {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }));
                }
            },
            (Key::Named(NamedKey::Tab), true, false, false) => {
                if context.curr_popup.is_none() {
                    context.is_chat_input_focused = true;
                    return Ok(focus(context.chat_input_id.clone()));
                }
            },
            (Key::Character("c"), true, false, false) => {
                if context.copy_buffer.is_some() {
                    return Ok(Task::done(IcedMessage::CopyPopupContent));
                }
            },
            (Key::Character("f"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Find { re: context.find_pattern.as_ref().map(|(pattern, _)| pattern.to_string()), error: None }, prev: None }));
                }
            },
            (Key::Character("h"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Help, prev: None }));
                }
            },
            (Key::Character("l"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }));
                }
            },
            (Key::Character("u"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }));
                }
            },
            (Key::Character("-"), true, false, false) => {
                return Ok(context.zoom_out());
            },
            (Key::Character("="), true, false, false) => {
                return Ok(context.zoom_in());
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

                context.close_popup();
            },
            m => {
                return Ok(api_key::update(&mut context.get_api_keys_context, m).map(IcedMessage::GetApiKeys));
            },
        },
        IcedMessage::ChatViewScrolled(o) => {
            context.chat_view_scrolled = o;
        },
        IcedMessage::OpenPopup { curr, prev } => {
            if let Popup::Image(_) | Popup::Find { .. } | Popup::FileSelector = &curr {
                return Ok(Task::done(IcedMessage::Notify(String::from("Not implemented yet"))));
            }

            let mut tasks: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled),
            ];

            // if curr.has_long_text_input() {
            //     tasks.push(focus(context.long_text_editor_id.clone()));
            // }

            if curr.has_short_text_input() {
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
            return Ok(scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled));
        },
        IcedMessage::CopyPopupContent => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::CopyString(s) => {
            return Ok(iced::clipboard::write(s));
        },
        IcedMessage::ChangeTitle => {
            let new_title = context.short_text_editor_content.to_string();
            let old_title = context.chat.title.clone();

            if !new_title.is_empty() {
                context.chat.title = Some(new_title);
            }

            if context.chat.title != old_title {
                context.chat.store(&context.global_index_dir)?;
            }

            context.close_popup();
            return Ok(scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled));
        },
        IcedMessage::SetChatConfig(c) => {
            // NOTE: You can't change the system prompt in this ui.
            set_chat_config(&mut context.chat.config, &[], c);
            context.chat.store(&context.global_index_dir)?;
        },
        IcedMessage::EditShortText(s) => {
            context.short_text_editor_content = s;
        },
        IcedMessage::EditChatInput(a) => {
            context.chat_input_content.perform(a);
        },
        IcedMessage::IsChatInputFocused(f) => {
            context.is_chat_input_focused = f;
        },
        IcedMessage::HoverChatButton => {
            context.is_chat_button_hovered = true;

            // unfocus the text editor
            return Ok(focus(context.chat_view_id.clone()));
        },
        IcedMessage::UnhoverChatButton => {
            context.is_chat_button_hovered = false;
        },
        IcedMessage::Send => {
            let query = context.chat_input_content.text();

            if query.is_empty() {
                return Ok(Task::none());
            }

            let query = vec![LLMToken::String(query)];
            let job_id = JobId::new();
            context.bg_at = Some(Local::now().timestamp_millis());
            context.bg_job = Some(job_id);
            context.bg_error = None;
            context.curr_processing_tokens = Some(query.clone());
            context.is_chat_button_hovered = false;

            return Ok(Task::batch(vec![
                Task::done(IcedMessage::BackgroundJob(Job {
                    id: job_id,
                    kind: JobKind::AddChatTurn {
                        chat_id: context.chat.id,
                        api_keys: context.api_keys.clone(),
                        query,
                    },
                })),
                snap_to(context.chat_view_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }),
            ]));
        },
        IcedMessage::Error(_) => unreachable!(),
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(job_result) => match &job_result.kind {
            JobResultKind::AddChatTurnSuccess if context.bg_job.is_some() && context.bg_job == job_result.id => {
                context.bg_at = None;
                context.bg_job = None;
                context.bg_error = None;
                context.curr_processing_tokens = None;
                context.set_chat_input_content(String::new());
            },
            JobResultKind::AddChatTurnError(e) if context.bg_job.is_some() && context.bg_job == job_result.id => {
                context.bg_at = None;
                context.bg_job = None;
                context.bg_error = Some(e.to_string());
                context.curr_processing_tokens = None;
            },
            _ => {},
        },
        IcedMessage::Notify(_) => unreachable!(),
        IcedMessage::Focus => {
            let mut tasks = vec![scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled)];

            if let Some(Popup::GetApiKeys) = context.curr_popup {
                tasks.push(context.get_api_keys_context.focus().map(IcedMessage::GetApiKeys));
            }

            return Ok(Task::batch(tasks));
        },
        IcedMessage::PrepareScratchPad => {
            let content = match (&context.loaded_image, &context.copy_buffer) {
                (Some(id), _) => ScratchPadContent::Image { path: id.path(&context.working_dir)? },
                (_, Some(s)) => ScratchPadContent::Text { content: s.to_string(), extension: context.syntax_highlight.clone() },
                (None, None) => unreachable!(),
            };

            return Ok(Task::done(IcedMessage::OpenScratchPad { title: context.popup_title.clone(), content }));
        },
        IcedMessage::OpenScratchPad { .. } => unreachable!(),
        IcedMessage::Kill => {
            return Ok(Task::done(IcedMessage::Dead));
        },
        IcedMessage::Dead => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut turns: Vec<Element<IcedMessage>> = Vec::with_capacity(context.chat.turns.len());

    for turn_id in context.chat.turns.iter() {
        let turn = context.turns.get(turn_id).unwrap();
        turns.push(render_turn(
            "User",
            turn.user_at,
            &turn.user,
            &None,
            &[],
            context.user_code_blocks.get(turn_id),
            turn.api.request_body.clone(),
            Horizontal::Right,
            context,
        ));
        turns.push(render_turn(
            turn.model.short_name(),
            turn.assistant_at,
            &[LLMToken::String(turn.assistant.to_string())],
            &turn.thinking,
            &turn.web_search_results,
            context.ai_code_blocks.get(turn_id),
            turn.api.response_body.clone(),
            Horizontal::Left,
            context,
        ));
    }

    if let Some(tokens) = &context.curr_processing_tokens {
        turns.push(render_turn(
            "User",
            context.bg_at.unwrap(),
            tokens,
            &None,
            &[],
            None,
            None,
            Horizontal::Right,
            context,
        ));
    }

    if context.bg_job.is_some() {
        turns.push(text!("Processing...").color(white()).size(context.zoom * 14.0).into());
    }

    if let Some(error) = &context.bg_error {
        turns.push(text!("{error}").color(red()).size(context.zoom * 14.0).into());
    }

    // Without this, chat_input will hide the error message
    turns.push(text!("").width(context.window_size.width).height(context.window_size.height * 0.3).into());

    let turns_stretched = Column::from_vec(turns)
        .padding(context.zoom * 20.0)
        .spacing(context.zoom * 20.0);

    let mut turns_scrollable = Scrollable::new(turns_stretched).id(context.chat_view_id.clone());

    if context.curr_popup.is_none() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::ChatViewScrolled(v.absolute_offset()));
    }

    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));
    let mut full_view = vec![
        Row::from_vec(vec![
            text!("{}", context.chat.title.as_ref().unwrap_or(&String::from("(untitled)"))).color(white()).size(context.zoom * 18.0).into(),
            if context.curr_popup.is_some() {
                disabled_button("Change", blue(), context.zoom).into()
            } else {
                button("Change", IcedMessage::OpenPopup { curr: Popup::ChangeTitle, prev: None }, blue(), context.zoom).into()
            },
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 16.0)
            .align_y(Vertical::Center)
            .into(),
        render_buttons(context),
    ];

    if let Some((pattern, _)) = &context.find_pattern {
        let matches = context.chat.turns.iter().map(
            |turn_id| *context.find_result.get(turn_id).unwrap_or(&0)
        ).sum::<usize>();
        full_view.push(text!(
            "find: {pattern:?}, found {matches} result{}",
            if matches == 1 { "" } else { "s" },
        ).color(white()).size(context.zoom * 14.0).into());
    }

    full_view.push(turns_colored.into());

    // Without this, chat_input won't be seen if there are too small turns
    full_view.push(Container::new(
        Space::new().width(context.window_size.width).height(context.window_size.height)
    ).style(|_| set_bg(black())).into());

    let full_view = Column::from_vec(full_view);
    let full_view = Container::new(full_view).style(|_| set_bg(gray(0.16)));
    let chat_config_ui_in_container = Container::new(
        Column::from_vec(vec![
            Row::from_vec(vec![
                button("Attach", IcedMessage::OpenPopup { curr: Popup::FileSelector, prev: None }, skyblue(), context.zoom).into(),
                chat_config_ui1(&context.chat.config, context.zoom).map(IcedMessage::SetChatConfig).into(),
            ])
                .spacing(context.zoom * 8.0)
                .align_y(Vertical::Center)
                .into(),
        ])
            .padding(Padding { right: context.zoom * 8.0, ..Padding::ZERO })
            .width(context.window_size.width)
            .align_x(Horizontal::Right)
    )
        .style(|_| set_bg(gray(0.35)))
        .into();
    let full_view_with_text_input = Stack::from_vec(vec![
        full_view.into(),
        chat_ui(
            context.is_chat_input_focused,
            context.curr_popup.is_none() && context.bg_job.is_none(),
            context.is_chat_button_hovered,
            context.chat_input_id.clone(),
            &context.chat_input_content,
            "Send",
            chat_config_ui_in_container,
            context.zoom * 48.0,
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

    else if let Some(Popup::WebSearchResults(s)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(render_web_search_results(s, context), context),
        ]).into();
    }

    else if let Some(Popup::ChangeTitle) = context.curr_popup {
        let change_title = Row::from_vec(vec![
            TextInput::new("", &context.short_text_editor_content)
                .id(context.short_text_editor_id.clone())
                .size(context.zoom * 14.0)
                .width(context.zoom * 512.0)
                .on_input(IcedMessage::EditShortText)
                .on_submit(IcedMessage::ChangeTitle)
                .into(),
            button("Change", IcedMessage::ChangeTitle, blue(), context.zoom).into(),
        ]).spacing(context.zoom * 8.0);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(change_title.into(), context),
        ]).into();
    }

    else if let Some(logs) = &context.loaded_logs {
        let view = render_logs(logs, context, context.popup_scroll_id.clone(), context.zoom);
        full_view_stacked = Stack::from_vec(vec![full_view_stacked, view]).into();
    }

    else if let Some(token_usage) = &context.loaded_token_usage {
        let view = render_token_usage(token_usage, context, context.popup_scroll_id.clone(), context.zoom);
        full_view_stacked = Stack::from_vec(vec![full_view_stacked, view]).into();
    }

    else if let Some(Popup::Log(_) | Popup::Help | Popup::Thinking(_) | Popup::CodeBlock { .. }) = &context.curr_popup {
        let title = text!("{}", context.popup_title.clone().unwrap_or(String::new()))
            .color(white())
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
    let buttons = vec![
        button("See (l)ogs", IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }, yellow(), context.zoom),
        button("Token (u)sage", IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }, yellow(), context.zoom),
        button("(F)ind", IcedMessage::OpenPopup { curr: Popup::Find { re: context.find_pattern.as_ref().map(|(pattern, _)| pattern.to_string()), error: None }, prev: None }, blue(), context.zoom),
        button("(H)elp", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink(), context.zoom),
    ];

    let buttons = if context.curr_popup.is_some() {
        buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons.into_iter().map(|button| button.into()).collect()
    };

    Row::from_vec(buttons).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into()
}

pub fn render_turn<'n, 'cn, 'cx>(
    name: &'n str,
    timestamp: i64,
    content: &'cn [LLMToken],
    thinking: &Option<String>,
    web_search_results: &[WebSearchResult],
    code_blocks: Option<&Vec<(String, Option<String>)>>,
    raw_api: Option<LogId>,
    align_name: Horizontal,
    context: &'cx IcedContext,
) -> Element<'cx, IcedMessage> {
    let stringified_llm_tokens = stringify_llm_tokens(content);
    let mut buttons = vec![
        button("Copy", IcedMessage::CopyString(stringified_llm_tokens.to_string()), blue(), context.zoom),
    ];

    if let Some(thinking) = thinking {
        buttons.push(button("Thinking", IcedMessage::OpenPopup { curr: Popup::Thinking(thinking.to_string()), prev: None }, black(), context.zoom));
    }

    if !web_search_results.is_empty() {
        buttons.push(button("Web Search", IcedMessage::OpenPopup { curr: Popup::WebSearchResults(web_search_results.to_vec()), prev: None }, black(), context.zoom));
    }

    if let Some(raw_api) = raw_api {
        buttons.push(button("Api", IcedMessage::OpenPopup { curr: Popup::Log((String::new(), raw_api)), prev: None }, yellow(), context.zoom));
    }

    if let Some(code_blocks) = code_blocks {
        for (i, (code, ext)) in code_blocks.iter().enumerate() {
            buttons.push(button(&format!("Code Block {}", i + 1), IcedMessage::OpenPopup { curr: Popup::CodeBlock { code: code.to_string(), ext: ext.clone() }, prev: None }, yellow(), context.zoom));
        }
    }

    buttons.push(button(
        "Scratch Pad",
        IcedMessage::OpenScratchPad { title: None, content: ScratchPadContent::Text { content: stringified_llm_tokens.to_string(), extension: Some(String::from("md")) } },
        purple(),
        context.zoom,
    ));

    let mut buttons: Vec<Element<IcedMessage>> = if context.curr_popup.is_some() {
        buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons.into_iter().map(|button| button.into()).collect()
    };

    // If it were `buttons.len() < 7`, it would be too ugly when there are 7 buttons.
    let buttons: Element<IcedMessage> = if buttons.len() < 8 {
        Row::from_vec(buttons).spacing(context.zoom * 8.0).into()
    } else {
        // Let's hope that there are less than 13 buttons...
        Column::from_vec(vec![
            Row::from_vec(buttons.drain(0..6).collect()).spacing(context.zoom * 8.0).into(),
            Row::from_vec(buttons).spacing(context.zoom * 8.0).into(),
        ]).spacing(context.zoom * 8.0).into()
    };

    Container::new(Column::from_vec(vec![
        Column::from_vec(vec![
            text!("{name}").color(white()).size(context.zoom * 18.0).into(),
            text!("({})", prettify_timestamp(timestamp)).color(white()).size(context.zoom * 14.0).into(),
        ])
            .width(context.window_size.width)
            .align_x(align_name)
            .into(),
        Column::from_vec(vec![
            Container::new(
                Space::new()
                    .width(context.window_size.width * 0.4)
                    .height(context.zoom * 4.0),
            )
                .style(|_| set_bg(white()))
                .into(),
        ])
            .width(context.window_size.width)
            .align_x(align_name)
            .into(),
        Space::new().height(context.zoom * 12.0).into(),
        render_llm_tokens(
            content.to_vec(),
            "",  // TODO: working dir
            context.zoom,
            context,
        )
            .width(context.window_size.width)
            .align_x(Horizontal::Left)
            .into(),
        buttons,
    ]).spacing(context.zoom * 8.0))
        .padding(context.zoom * 8.0)
        .style(|_| set_round_bg(gray(0.25), context.zoom))
        .into()
}

fn render_web_search_results<'s, 'c>(results: &'s [WebSearchResult], context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let results: Vec<Element<IcedMessage>> = results.iter().map(
        |search_result| {
            let mut elements = vec![
                text!("{}", search_result.title.clone().unwrap_or(String::from("(untitled)")))
                    .width(context.window_size.width)
                    .size(context.zoom * 18.0)
                    .into(),
            ];

            if let Some(summary) = &search_result.summary {
                elements.push(text!("{summary}").size(context.zoom * 14.0).into());
            }

            if let Some(url) = &search_result.url {
                elements.push(Row::from_vec(vec![
                    text!("{url}").size(context.zoom * 14.0).into(),
                    button("Copy", IcedMessage::CopyString(url.to_string()), blue(), context.zoom).into(),
                ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into());
            }

            Container::new(Column::from_vec(elements).spacing(context.zoom * 8.0))
                .padding(context.zoom * 8.0)
                .style(|_| set_round_bg(gray(0.15), context.zoom))
                .into()
        }
    ).collect();
    Scrollable::new(Column::from_vec(results).spacing(context.zoom * 12.0)).id(context.popup_scroll_id.clone()).into()
}

fn get_missing_api_keys(api_keys: &HashMap<String, String>, model: Model) -> Vec<(String, Vec<String>)> {
    let env_var = model.api_key_env_var();

    if std::env::var(env_var).is_err() && !api_keys.contains_key(env_var) {
        vec![(env_var.to_string(), vec![])]  // There's no `agent_name`!
    } else {
        vec![]
    }
}

pub trait ChatMessage {
    fn hover_button() -> Self;
    fn unhover_button() -> Self;
    fn edit(action: TextEditorAction) -> Self;
    fn enter() -> Self;
}

pub fn chat_ui<'c, Message: ChatMessage + Clone + 'c>(
    is_focused: bool,
    can_be_focused: bool,
    is_button_hovered: bool,
    editor_id: Id,
    editor_content: &'c TextEditorContent,
    enter_button: &'static str,
    chat_config_ui: Element<'c, Message>,
    chat_config_ui_height: f32,
    mut window_size: Size,
    zoom: f32,
) -> Element<'c, Message> {
    window_size.height -= 52.0;  // size of the tabs
    let mut text_editor = TextEditor::new(editor_content)
        .size(zoom * 14.0)
        .id(editor_id)
        .width(window_size.width * 0.75)
        .height(zoom * if is_focused { 256.0 } else { 32.0 });

    if can_be_focused {
        text_editor = text_editor
            .on_action(|action| Message::edit(action))
            .key_binding(move |key_press| {
                let KeyPress { key, modifiers, .. } = &key_press;

                match (key.as_ref(), modifiers.control()) {
                    (Key::Named(NamedKey::Enter), true) => Some(Binding::Sequence(vec![Binding::Unfocus, Binding::Custom(Message::enter())])),
                    (Key::Named(NamedKey::Tab), true) => Some(Binding::Unfocus),
                    _ => Binding::from_key_press(key_press),
                }
            });
    }

    // If it uses iced::widget::Button, the user has to click the button twice:
    // once for unfocusing the text_editor and again for the button.
    // -> This is extremely bothering, so I came up with this work-around.
    let button_width = zoom * enter_button.len() as f32 * 10.0 + 25.0;
    let button_height = zoom * 32.0;
    let mut button: Element<Message> = Container::new(
        Row::from_vec(vec![
            Column::from_vec(vec![
                text!("{enter_button}").color(white()).size(zoom * 14.0).into()
            ])
                .width(Length::Fill)
                .align_x(Horizontal::Center)
                .into()
        ])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Vertical::Center)
    ).style(move |_| {
        let bg_color = if !can_be_focused {
            gray(0.6)
        } else if is_button_hovered {
            let (r, g, b) = (blue().r, blue().g, blue().b);
            Color::from_rgba(r, g, b, 0.5)
        } else {
            blue()
        };

        set_round_bg(bg_color, zoom)
    })
        .width(button_width)
        .height(button_height)
        .into();

    if can_be_focused {
        button = MouseArea::new(button)
            .on_enter(Message::hover_button())
            .on_exit(Message::unhover_button())
            .on_press(Message::enter())
            .into();
    }

    let container_height = if is_focused { zoom * 268.0 } else { zoom * 44.0 };
    let text_editor = Container::new(
        Row::from_vec(vec![
            Space::new().width(window_size.width * 0.05).into(),
            text_editor.into(),
            Space::new().width((window_size.width * 0.2 - button_width) / 2.0).into(),
            button,
        ])
            .padding(Padding { bottom: zoom * 6.0, ..Padding::ZERO })
            .height(Length::Fill)
            .align_y(Vertical::Bottom)
    )
        .width(window_size.width)
        .height(container_height)
        .style(|_| set_bg(gray(0.35)));

    Column::from_vec(vec![
        Space::new().height(window_size.height - container_height - chat_config_ui_height).into(),
        chat_config_ui,
        text_editor.into(),
    ]).into()
}

static CODE_BLOCK_START_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^```([a-zA-Z0-9]*)$").unwrap());

fn extract_code_blocks(turn: &str) -> Vec<(String, Option<String>)> {
    let mut inside_code_block = false;
    let mut code_blocks = vec![];
    let mut curr_block = vec![];
    let mut curr_ext = None;

    for line in turn.lines() {
        if !inside_code_block && let Some(cap) = CODE_BLOCK_START_RE.captures(line) {
            inside_code_block = true;
            let ext = cap.get(1).unwrap().as_str().to_string();

            if !ext.is_empty() {
                curr_ext = Some(ext);
            }
        }

        else if inside_code_block && line == "```" {
            code_blocks.push((curr_block.join("\n"), curr_ext));
            curr_block = vec![];
            curr_ext = None;
            inside_code_block = false;
        }

        else if inside_code_block {
            curr_block.push(line);
        }
    }

    code_blocks
}
