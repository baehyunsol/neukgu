use super::{button, green};
use super::popup::{PopupContext, PopupMessage, into_popup};
use iced::{Element, Length, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Row, TextInput, text};

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub missing_api_keys: Vec<String>,
    pub key1: String,
    pub key2: String,
    pub key3: String,
}

impl IcedContext {
    pub fn new(missing_api_keys: Vec<String>) -> IcedContext {
        IcedContext {
            missing_api_keys,
            key1: String::new(),
            key2: String::new(),
            key3: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    EditKey1(String),
    EditKey2(String),
    EditKey3(String),
    Enter,
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
        IcedMessage::Enter => {},
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
        value: &'m str,
        on_input: F,
        zoom: f32,
    ) -> Element<'m, IcedMessage> {
        let s = format!("{}{env_var}:", " ".repeat(32 - env_var.len()));

        Row::from_vec(vec![
            text!("{s}").size(zoom * 14.0).into(),
            TextInput::new("", value)
                .size(zoom * 14.0)
                .width(zoom * 256.0)
                .on_input(on_input)
                .into(),
        ]).align_y(Vertical::Center).spacing(zoom * 8.0).into()
    }

    let mut column: Vec<Element<IcedMessage>> = vec![text!("Enter API Keys").size(zoom * 18.0).into()];
    column.push(input(&api_keys_context.missing_api_keys[0], &api_keys_context.key1, IcedMessage::EditKey1, zoom));

    if let Some(env_var) = api_keys_context.missing_api_keys.get(1) {
        column.push(input(env_var, &api_keys_context.key2, IcedMessage::EditKey2, zoom));
    }

    if let Some(env_var) = api_keys_context.missing_api_keys.get(2) {
        column.push(input(env_var, &api_keys_context.key3, IcedMessage::EditKey3, zoom));
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
