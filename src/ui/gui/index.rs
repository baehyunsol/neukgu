use super::{black, button, green, white};
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Scrollable, Space, text};
use iced::widget::container::{Container, Style};

pub struct IcedContext {
    pub window_size: Size,
    pub recent_projects: Vec<(/* TODO */)>,
    pub current_tabs: Vec<(/* TODO */)>,
    pub zoom: f32,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    NewTab,
    OpenPopup,
}

pub fn boot() -> IcedContext {
    IcedContext {
        window_size: Size::new(0.0, 0.0),
        recent_projects: vec![],
        current_tabs: vec![],
        zoom: 1.0,
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("-"), true, false, false) => {
                context.zoom = context.zoom.max(0.2) - 0.1;
            },
            (Key::Character("="), true, false, false) => {
                context.zoom = context.zoom.min(2.4) + 0.1;
            },
            _ => {},
        },
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::NewTab => unreachable!(),
        IcedMessage::OpenPopup => todo!(),
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec(vec![
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Recent projects").size(context.zoom * 14.0).into(),
                button("New project", IcedMessage::OpenPopup, green(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into(),
            Container::new(Scrollable::new(render_projects(context)).width(Length::Fill)).style(
                |_| Style {
                    background: Some(Background::Color(black())),
                    border: Border {
                        color: white(),
                        width: 0.0,
                        radius: Radius::new(8.0),
                    },
                    ..Style::default()
                }
            ).width(Length::Fill).height(Length::Fill).into(),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
            .width(context.window_size.width)
            .height(context.window_size.height * 0.43)
            .into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Current tabs").size(context.zoom * 14.0).into(),
                button("New tab", IcedMessage::NewTab, green(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into(),
            Container::new(Scrollable::new(render_tabs(context)).width(Length::Fill)).style(
                |_| Style {
                    background: Some(Background::Color(black())),
                    border: Border {
                        color: white(),
                        width: 0.0,
                        radius: Radius::new(8.0),
                    },
                    ..Style::default()
                }
            ).width(Length::Fill).height(Length::Fill).into(),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
            .width(context.window_size.width)
            .height(context.window_size.height * 0.43)
            .into(),
    ]).spacing(context.zoom * 8.0).into()
}

fn render_projects<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec((0..20).map(|_| text!("PLACEHOLDER").into()).collect()).into()
}

fn render_tabs<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec((0..20).map(|_| text!("PLACEHOLDER").into()).collect()).into()
}
