use super::{black, blue, button, purple, red, set_bg, white};
use iced::{Background, Color, Element, Length};
use iced::border::{Border, Radius};
use iced::widget::{Column, Row};
use iced::widget::container::{Container, Style};

pub trait PopupContext {
    fn can_close_popup(&self) -> bool;
    fn has_prev_popup(&self) -> bool;
    fn has_something_to_copy(&self) -> bool;
    fn can_open_scratch_pad(&self) -> bool;
    fn zoom(&self) -> f32;
}

pub trait PopupMessage {
    fn close_popup() -> Self;
    fn back_popup() -> Self;
    fn copy_popup_content() -> Self;
    fn open_scratch_pad() -> Self;
}

pub fn into_popup<'e, 'c, Message: Clone + PopupMessage + 'e, Context: PopupContext>(element: Element<'e, Message>, context: &'c Context) -> Element<'e, Message> {
    let mut buttons: Vec<Element<Message>> = vec![];
    let zoom = context.zoom();

    // There are some popups that cannot be closed (e.g. llm request).
    if context.can_close_popup() {
        buttons.push(button("X", Message::close_popup(), red(), zoom).into());
    }

    if context.has_prev_popup() {
        buttons.push(button("Back", Message::back_popup(), blue(), zoom).into());
    }

    if context.has_something_to_copy() {
        buttons.push(button("Copy", Message::copy_popup_content(), blue(), zoom).into());
    }

    if context.can_open_scratch_pad() {
        buttons.push(button("Scratch Pad", Message::open_scratch_pad(), purple(), zoom).into());
    }

    Container::new(
        Container::new(Column::from_vec(vec![
            Row::from_vec(buttons).padding(zoom * 8.0).spacing(zoom * 8.0).into(),
            element,
        ]).width(Length::Fill)).style(
            move |_| Style {
                background: Some(Background::Color(black())),
                border: Border {
                    color: white(),
                    width: zoom * 4.0,
                    radius: Radius::new(zoom * 8.0),
                },
                ..Style::default()
            }
        )
        .padding(zoom * 8.0)
        .width(Length::Fill)
    )
    .style(|_| set_bg(Color::from_rgba(0.0, 0.0, 0.0, 0.5)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(zoom * 32.0)
    .into()
}
