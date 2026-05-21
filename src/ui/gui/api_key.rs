use super::{button, green};
use super::popup::{PopupContext, PopupMessage, into_popup};
use iced::{Element, Length, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Id, Row, TextInput, text};
use iced::widget::operation::focus;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub missing_api_keys: Vec<String>,
    pub text_input_ids: Vec<Id>,
    pub key1: String,
    pub key2: String,
    pub key3: String,
    pub key4: String,
}

impl IcedContext {
    pub fn new(missing_api_keys: Vec<String>) -> IcedContext {
        IcedContext {
            missing_api_keys,
            text_input_ids: (0..4).map(|_| Id::unique()).collect(),
            key1: String::new(),
            key2: String::new(),
            key3: String::new(),
            key4: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    EditKey1(String),
    EditKey2(String),
    EditKey3(String),
    EditKey4(String),
    Enter,
    Focus(Id),
}

// You can't close/back/copy this popup.
impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { unreachable!() }
    fn back_popup() -> Self { unreachable!() }
    fn copy_popup_content() -> Self { unreachable!() }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::EditKey1(key) => {
            context.key1 = key;
        },
        IcedMessage::EditKey2(key) => {
            context.key2 = key;
        },
        IcedMessage::EditKey3(key) => {
            context.key3 = key;
        },
        IcedMessage::EditKey4(key) => {
            context.key4 = key;
        },
        IcedMessage::Enter => {},
        IcedMessage::Focus(id) => {
            return focus(id);
        },
    }

    Task::none()
}

pub fn get_api_keys_popup<'ac, 'pc, Context: PopupContext>(
    api_keys_context: &'ac IcedContext,
    popup_context: &'pc Context,
    zoom: f32,
) -> Element<'ac, IcedMessage> {
    fn input<'e, 'm, F: Fn(String) -> IcedMessage + 'm>(
        env_var: &'e str,
        id: Id,
        value: &'m str,
        on_input: F,
        on_submit: IcedMessage,
        zoom: f32,
    ) -> Element<'m, IcedMessage> {
        let s = format!("{}{env_var}:", " ".repeat(32 - env_var.len()));

        Row::from_vec(vec![
            text!("{s}").size(zoom * 14.0).into(),
            TextInput::new("", value)
                .id(id)
                .size(zoom * 14.0)
                .width(zoom * 256.0)
                .on_input(on_input)
                .on_submit(on_submit)
                .into(),
        ]).align_y(Vertical::Center).spacing(zoom * 8.0).into()
    }

    let mut column: Vec<Element<IcedMessage>> = vec![text!("Enter API Keys").size(zoom * 18.0).into()];
    column.push(input(
        &api_keys_context.missing_api_keys[0],
        api_keys_context.text_input_ids[0].clone(),
        &api_keys_context.key1,
        IcedMessage::EditKey1,
        if api_keys_context.missing_api_keys.len() == 1 {
            IcedMessage::Enter
        } else {
            IcedMessage::Focus(api_keys_context.text_input_ids[1].clone())
        },
        zoom,
    ));

    if let Some(env_var) = api_keys_context.missing_api_keys.get(1) {
        column.push(input(
            env_var,
            api_keys_context.text_input_ids[1].clone(),
            &api_keys_context.key2,
            IcedMessage::EditKey2,
            if api_keys_context.missing_api_keys.len() == 2 {
                IcedMessage::Enter
            } else {
                IcedMessage::Focus(api_keys_context.text_input_ids[2].clone())
            },
            zoom,
        ));
    }

    if let Some(env_var) = api_keys_context.missing_api_keys.get(2) {
        column.push(input(
            env_var,
            api_keys_context.text_input_ids[2].clone(),
            &api_keys_context.key3,
            IcedMessage::EditKey3,
            if api_keys_context.missing_api_keys.len() == 3 {
                IcedMessage::Enter
            } else {
                IcedMessage::Focus(api_keys_context.text_input_ids[3].clone())
            },
            zoom,
        ));
    }

    if let Some(env_var) = api_keys_context.missing_api_keys.get(3) {
        column.push(input(
            env_var,
            api_keys_context.text_input_ids[3].clone(),
            &api_keys_context.key4,
            IcedMessage::EditKey4,
            IcedMessage::Enter,
            zoom,
        ));
    }

    column.push(button("Enter", IcedMessage::Enter, green(), zoom).into());

    into_popup(
        Column::from_vec(column)
            .align_x(Horizontal::Center)
            .width(Length::Fill)
            .spacing(zoom * 12.0)
            .into(),
        popup_context,
    )
}
