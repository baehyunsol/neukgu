use super::{button, green, red};
use iced::{Element, Size, Task};
use iced::alignment::Horizontal;
use iced::widget::{Column, Sensor, text};

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub message: String,
    pub window_size: Size,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    WindowResized(Size),
    Okay,
}

pub fn boot(message: String) -> IcedContext {
    IcedContext {
        message,
        window_size: Size::new(0.0, 0.0),
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::Okay => unreachable!(),
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Sensor::new(
        Column::from_vec(vec![
            text!("Error").color(red()).into(),
            text!("{}", context.message).into(),
            button("Okay", IcedMessage::Okay, green()).into(),
        ])
            .padding(20)
            .spacing(20)
            .align_x(Horizontal::Center)
    )
        .on_show(|s| IcedMessage::WindowResized(s))
        .on_resize(|s| IcedMessage::WindowResized(s))
        .into()
}
