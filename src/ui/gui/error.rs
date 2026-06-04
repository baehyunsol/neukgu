use super::{blue, button, red};
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

impl IcedContext {
    pub fn new(message: String, window_size: Size, zoom: f32) -> IcedContext {
        IcedContext {
            message,
            window_size,
            zoom,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },

    // Kill: The caller wants to kill this tab.
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    Dead,
}

pub fn update(_: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("y"), true, false, false) => Task::done(IcedMessage::Dead),
            _ => Task::none(),
        },
        IcedMessage::Kill => Task::done(IcedMessage::Dead),
        IcedMessage::Dead => unreachable!(),
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec(vec![
        text!("ERROR").size(context.zoom * 21.0).color(red()).into(),
        text!("{}", context.message).size(context.zoom * 14.0).into(),
        button("Oka(y)", IcedMessage::Dead, blue(), context.zoom).into(),
    ])
        .padding(context.zoom * 20.0)
        .spacing(context.zoom * 20.0)
        .align_x(Horizontal::Center)
        .width(Length::Fill)
        .into()
}
