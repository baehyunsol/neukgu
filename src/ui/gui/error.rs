use super::{button, green, red};
use iced::{Element, Length, Size, Task};
use iced::alignment::Horizontal;
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, text};

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub message: String,
    pub window_size: Size,
    pub zoom: f32,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },
    Okay,
}

pub fn update(_: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("y"), false, false, false) => {
                return Task::done(IcedMessage::Okay);
            },
            _ => {
                return Task::none();
            },
        },
        IcedMessage::Okay => unreachable!(),
    }
}

pub fn boot(message: String, window_size: Size, zoom: f32) -> IcedContext {
    IcedContext {
        message,
        window_size,
        zoom,
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec(vec![
        text!("Error").size(context.zoom * 18.0).color(red()).into(),
        text!("{}", context.message).size(context.zoom * 14.0).into(),
        button("Oka(y)", IcedMessage::Okay, green(), context.zoom).into(),
    ])
        .padding(context.zoom * 20.0)
        .spacing(context.zoom * 20.0)
        .align_x(Horizontal::Center)
        .width(Length::Fill)
        .into()
}
