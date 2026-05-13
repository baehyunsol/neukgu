use super::{
    blue,
    button,
    disabled_button,
    gray,
    set_bg,
};
use super::worker::{
    Job,
    JobId,
    JobKind,
    JobResult,
};
use crate::{
    Chat,
    ChatId,
    ChatTurn,
    ChatTurnId,
    Error,
    LLMToken,
    get_global_index_dir,
};
use iced::{Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, Container, Id, Row, Space, Stack};
use iced::widget::operation::{AbsoluteOffset, is_focused, scroll_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Binding,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    KeyPress,
    TextEditor,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub chat: Chat,
    pub turns: HashMap<ChatTurnId, ChatTurn>,

    // If it's `Some(_)`, that means the LLM is working in background,
    // so the user cannot send a new message.
    pub bg_job: Option<JobId>,

    pub window_size: Size,
    pub chat_view_id: Id,
    pub chat_input_id: Id,
    pub chat_view_scrolled: AbsoluteOffset,
    pub curr_popup: Option<Popup>,
    pub chat_input_content: TextEditorContent,
    pub is_chat_input_focused: bool,
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
            chat,
            turns,
            bg_job: None,
            window_size,
            chat_view_id: Id::unique(),
            chat_input_id: Id::unique(),
            chat_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            curr_popup: None,
            chat_input_content: TextEditorContent::new(),
            is_chat_input_focused: false,
            zoom: 1.0,
        })
    }

    pub fn set_chat_input_content(&mut self, c: String) {
        self.chat_input_content.perform(TextEditorAction::SelectAll);
        self.chat_input_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.chat_input_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    ChatViewScrolled(AbsoluteOffset),
    EditChatInput(TextEditorAction),
    IsChatInputFocused(bool),
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

impl ChatMessage for IcedMessage {
    fn edit(action: TextEditorAction) -> IcedMessage {
        IcedMessage::EditChatInput(action)
    }

    fn enter() -> IcedMessage {
        IcedMessage::Send
    }
}

#[derive(Clone, Debug)]
pub enum Popup {}

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
                // TODO
            }

            return Ok(is_focused(context.chat_input_id.clone()).map(|is_focused| IcedMessage::IsChatInputFocused(is_focused)));
        },
        IcedMessage::KeyPressed { key, modifiers } => todo!(),
        IcedMessage::ChatViewScrolled(o) => {
            context.chat_view_scrolled = o;
        },
        IcedMessage::EditChatInput(a) => {
            context.chat_input_content.perform(a);
        },
        IcedMessage::IsChatInputFocused(f) => {
            context.is_chat_input_focused = f;
        },
        IcedMessage::Send => {
            let job_id = JobId::new();
            context.bg_job = Some(job_id);
            let query = context.chat_input_content.text();
            context.set_chat_input_content(String::new());
            return Ok(Task::done(IcedMessage::BackgroundJob(Job {
                id: job_id,
                kind: JobKind::AddChatTurn { chat_id: context.chat.id, query: vec![LLMToken::String(query)] },
            })));
        },
        IcedMessage::Error(_) => unreachable!(),
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(_) => todo!(),
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
    let full_view = vec![];
    let full_view = Column::from_vec(full_view).width(context.window_size.width).height(context.window_size.height);
    let full_view_with_text_input = Stack::from_vec(vec![
        full_view.into(),
        chat_ui(
            context.is_chat_input_focused,
            context.curr_popup.is_none() && context.bg_job.is_none(),
            context.chat_input_id.clone(),
            &context.chat_input_content,
            "Send",
            context.window_size,
            context.zoom,
        ),
    ]);

    full_view_with_text_input.into()
}

pub trait ChatMessage {
    fn edit(action: TextEditorAction) -> Self;
    fn enter() -> Self;
}

pub fn chat_ui<'c, Message: ChatMessage + Clone + 'c>(
    is_focused: bool,
    can_be_focused: bool,
    editor_id: Id,
    editor_content: &'c TextEditorContent,
    enter_button: &'static str,
    window_size: Size,
    zoom: f32,
) -> Element<'c, Message> {
    let mut text_editor = TextEditor::new(editor_content)
        .id(editor_id)
        .width(window_size.width * 0.75)
        .height(window_size.height * if is_focused { 0.25 } else { 0.05 });

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

    let text_editor = Container::new(
        Row::from_vec(vec![
            Space::new().width(window_size.width * 0.05).into(),
            text_editor.into(),
            Space::new().width((window_size.width * 0.2 - zoom * 80.0) / 2.0).into(),
            if !can_be_focused {
                disabled_button(enter_button, blue(), zoom).into()
            } else {
                button(enter_button, Message::enter(), blue(), zoom).into()
            },
        ])
            .height(Length::Fill)
            .align_y(Vertical::Center)
    )
        .width(window_size.width)
        .height(window_size.height * if is_focused { 0.35 } else { 0.15 })
        .style(|_| set_bg(gray(0.35)));

    Column::from_vec(vec![
        Space::new().height(window_size.height * if is_focused { 0.65 } else { 0.85 }).into(),
        text_editor.into(),
    ]).into()
}
