use super::{
    black,
    blue,
    button,
    gray,
    pink,
    red,
    set_bg,
    set_round_bg,
    white,
    yellow,
};
use super::popup::{PopupContext, PopupMessage, into_popup};
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
    Thinking,
    WebSearchResult,
    get_global_index_dir,
    load_log,
    prettify_timestamp,
    stringify_llm_tokens,
};
use iced::{Color, Element, Length, Padding, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Checkbox, Column, Container, Id, MouseArea, PickList, Row, Scrollable, Space, Stack, text};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, focus, is_focused, scroll_to, snap_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Binding,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    KeyPress,
    TextEditor,
};
use ragit_fs::join4;
use regex::Regex;
use std::collections::hash_map::{Entry, HashMap};
use std::sync::Arc;

const HELP_MESSAGE: &str = "(TODO: write help message)";

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub chat: Chat,
    pub turns: HashMap<ChatTurnId, ChatTurn>,

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
    pub log_dir: String,
    pub chat_view_id: Id,
    pub popup_scroll_id: Id,
    pub chat_input_id: Id,
    pub chat_view_scrolled: AbsoluteOffset,
    pub find_pattern: Option<(String, Regex)>,
    pub find_result: HashMap<ChatTurnId, usize>,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub long_text_editor_content: TextEditorContent,
    pub chat_input_content: TextEditorContent,
    pub syntax_highlight: Option<String>,
    pub popup_title: Option<String>,
    pub is_chat_input_focused: bool,
    pub is_chat_button_hovered: bool,
    pub zoom: f32,
}

impl IcedContext {
    pub fn new(chat_id: ChatId, window_size: Size) -> Result<IcedContext, Error> {
        let global_index_dir = get_global_index_dir()?;
        let chat = Chat::load(chat_id, &global_index_dir)?;
        let mut turns = HashMap::with_capacity(chat.turns.len());

        for turn in chat.turns.iter() {
            turns.insert(*turn, ChatTurn::load(*turn, &global_index_dir)?);
        }

        Ok(IcedContext {
            chat: chat.clone(),
            turns,
            bg_at: None,
            bg_job: None,
            bg_error: None,
            curr_processing_tokens: None,
            window_size,
            global_index_dir: global_index_dir.to_string(),
            log_dir: join4(&global_index_dir, "chats", ".neukgu", "logs")?,
            chat_view_id: Id::unique(),
            popup_scroll_id: Id::unique(),
            chat_input_id: Id::unique(),
            chat_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            find_pattern: None,
            find_result: HashMap::new(),
            curr_popup: None,
            prev_popup: None,
            copy_buffer: None,
            long_text_editor_content: TextEditorContent::new(),
            chat_input_content: TextEditorContent::new(),
            syntax_highlight: None,
            popup_title: None,
            is_chat_input_focused: false,
            is_chat_button_hovered: false,
            zoom: 1.0,
        })
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
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
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_long_text_editor_content(HELP_MESSAGE.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::Thinking(thinking) => {
                self.copy_buffer = Some(thinking.to_string());
                self.set_long_text_editor_content(thinking.to_string());
                self.popup_title = Some(String::from("Thinking"));
            },
            Popup::WebSearchResults(_) => {},
            _ => todo!(),
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
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
}

impl PopupContext for IcedContext {
    fn can_close_popup(&self) -> bool { self.curr_popup.is_some() }
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

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    ChatViewScrolled(AbsoluteOffset),
    OpenPopup { curr: Popup, prev: Option<Popup> },
    BackPopup,
    ClosePopup,
    CopyPopupContent,
    CopyString(String),
    SelectModel(Model),
    ToggleThinking(bool),
    ToggleWebSearch(bool),
    EditChatInput(TextEditorAction),
    IsChatInputFocused(bool),
    HoverChatButton,
    UnhoverChatButton,
    Send,
    Error(String),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Focus,

    // Kill: The caller wants to kill this tab. This tab will show a popup "quit session?".
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    Dead,
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { IcedMessage::BackPopup }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
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
    Log((String, LogId)),
    TokenUsage,
    Help,
    Thinking(String),
    WebSearchResults(Vec<WebSearchResult>),
    Image(ImageId),
    Find { re: Option<String>, error: Option<String> },
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

                for turn in context.chat.turns.clone().iter() {
                    if let Entry::Vacant(e) = context.turns.entry(*turn) {
                        e.insert(ChatTurn::load(*turn, &context.global_index_dir)?);
                    }
                }
            }

            return Ok(is_focused(context.chat_input_id.clone()).map(|is_focused| IcedMessage::IsChatInputFocused(is_focused)));
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Escape), false, false, false) => {
                if context.curr_popup.is_some() {
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
            (Key::Character("u"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::TokenUsage, prev: None }));
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
        IcedMessage::ChatViewScrolled(o) => {
            context.chat_view_scrolled = o;
        },
        IcedMessage::OpenPopup { curr, prev } => {
            context.open_popup(curr)?;
            context.prev_popup = prev;
            return Ok(scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled));
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
        IcedMessage::SelectModel(model) => {
            context.chat.config.model = model;

            if !model.supports_web_search() {
                context.chat.config.enable_web_search = false;
            }

            context.chat.store(&context.global_index_dir)?;
        },
        IcedMessage::ToggleThinking(t) => {
            if t {
                context.chat.config.thinking = Thinking::Enabled;
            } else {
                context.chat.config.thinking = Thinking::Disabled;
            }

            context.chat.store(&context.global_index_dir)?;
        },
        IcedMessage::ToggleWebSearch(s) => {
            context.chat.config.enable_web_search = s;
            context.chat.store(&context.global_index_dir)?;
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
                    kind: JobKind::AddChatTurn { chat_id: context.chat.id, query },
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
        IcedMessage::Focus => {
            return Ok(scroll_to(context.chat_view_id.clone(), context.chat_view_scrolled));
        },
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
            Horizontal::Right,
            context,
        ));
    }

    if context.bg_job.is_some() {
        turns.push(text!("Processing...").size(context.zoom * 14.0).into());
    }

    if let Some(error) = &context.bg_error {
        turns.push(text!("{error}").size(context.zoom * 14.0).color(red()).into());
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
        Container::new(text!("{}", context.chat.title.as_ref().unwrap_or(&String::new())).size(context.zoom * 14.0)).padding(context.zoom * 8.0).into(),
        render_buttons(context),
    ];

    if let Some((pattern, _)) = &context.find_pattern {
        let matches = context.chat.turns.iter().map(
            |turn_id| *context.find_result.get(turn_id).unwrap_or(&0)
        ).sum::<usize>();
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
            context.is_chat_input_focused,
            context.curr_popup.is_none() && context.bg_job.is_none(),
            context.is_chat_button_hovered,
            context.chat_input_id.clone(),
            &context.chat_input_content,
            "Send",
            chat_config_ui(context),
            context.zoom * 48.0,
            context.window_size,
            context.zoom,
        ),
    ]);

    let mut full_view_stacked: Element<IcedMessage> = full_view_with_text_input.into();

    if let Some(Popup::WebSearchResults(s)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(render_web_search_results(s, context), context),
        ]).into();
    }

    else if let Some(Popup::Log(_) | Popup::Help | Popup::Thinking(_)) = &context.curr_popup {
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
    let buttons = vec![
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
    raw_api: Option<LogId>,
    align_name: Horizontal,
    context: &'cx IcedContext,
) -> Element<'cx, IcedMessage> {
    let mut buttons = vec![
        button("Copy", IcedMessage::CopyString(stringify_llm_tokens(content)), blue(), context.zoom),
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

    let buttons: Vec<Element<IcedMessage>> = if context.curr_popup.is_some() {
        buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons.into_iter().map(|button| button.into()).collect()
    };

    Container::new(Column::from_vec(vec![
        Column::from_vec(vec![
            text!("{name}").size(context.zoom * 18.0).into(),
            text!("({})", prettify_timestamp(timestamp)).size(context.zoom * 14.0).into(),
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
        Row::from_vec(buttons).spacing(context.zoom * 8.0).into(),
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

fn chat_config_ui<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Container::new(
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Model:").size(context.zoom * 14.0).into(),
                PickList::new(
                    Model::all().into_iter().filter(
                        |model| *model != Model::Mock && *model != Model::Disabled
                    ).collect::<Vec<_>>(),
                    Some(context.chat.config.model),
                    |model| IcedMessage::SelectModel(model),
                )
                    .text_size(context.zoom * 14.0)
                    .width(context.zoom * 160.0)
                    .into(),
                Checkbox::new(context.chat.config.thinking != Thinking::Disabled)
                    .label("Thinking")
                    .on_toggle(|t| IcedMessage::ToggleThinking(t))
                    .size(context.zoom * 14.0)
                    .text_size(context.zoom * 14.0)
                    .into(),
                Checkbox::new(context.chat.config.enable_web_search)
                    .label("Web Search")
                    .on_toggle_maybe(if context.chat.config.model.supports_web_search() {
                        Some(|s| IcedMessage::ToggleWebSearch(s))
                    } else {
                        None
                    })
                    .size(context.zoom * 14.0)
                    .text_size(context.zoom * 14.0)
                    .into(),
            ])
                .spacing(context.zoom * 8.0)
                .height(context.zoom * 48.0)
                .align_y(Vertical::Center)
                .into()
        ])
            .padding(Padding { right: context.zoom * 8.0, ..Padding::ZERO })
            .width(context.window_size.width)
            .align_x(Horizontal::Right),
    )
        .style(|_| set_bg(gray(0.35)))
        .into()
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
        .height(zoom * if is_focused { 160.0 } else { 32.0 });

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
                text!("{enter_button}").size(zoom * 14.0).into()
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

    let container_height = if is_focused { zoom * 172.0 } else { zoom * 44.0 };
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
