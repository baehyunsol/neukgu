use super::{button, green, red};
use iced::{Element, Size};
use iced::alignment::Horizontal;
use iced::widget::{Column, text};

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub message: String,
    pub window_size: Size,
    pub zoom: f32,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Okay,
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
        text!("Error").color(red()).into(),
        text!("{}", context.message).into(),
        button("Okay", IcedMessage::Okay, green(), context.zoom).into(),
    ])
        .padding(20)
        .spacing(20)
        .align_x(Horizontal::Center)
        .into()
}
