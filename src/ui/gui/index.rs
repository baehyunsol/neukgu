use super::{black, button, circle, green, red, white};
use super::tab::{TabId, TabPreview};
use chrono::Local;
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Scrollable, text};
use iced::widget::container::{Container, Style};

pub struct IcedContext {
    pub window_size: Size,
    pub recent_projects: Vec<(/* TODO */)>,
    pub current_tabs: Vec<TabPreview>,
    pub curr_popup: Option<(/* TODO */)>,
    pub zoom: f32,
    pub now: String,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    NewTab,
    OpenTab { id: TabId, index: usize },
    OpenPopup,
}

pub fn boot() -> IcedContext {
    IcedContext {
        window_size: Size::new(0.0, 0.0),
        recent_projects: vec![],
        current_tabs: vec![],
        curr_popup: None,
        zoom: 1.0,
        now: Local::now().to_rfc2822(),
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Tick => {
            context.now = Local::now().to_rfc2822();
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("p"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Task::done(IcedMessage::OpenPopup);
                }
            },
            (Key::Character("t"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Task::done(IcedMessage::NewTab);
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
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::NewTab => unreachable!(),
        IcedMessage::OpenTab { .. } => unreachable!(),
        IcedMessage::OpenPopup => todo!(),
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let c = Column::from_vec(vec![
        text!("{}", context.now).size(context.zoom * 14.0).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Recent projects").size(context.zoom * 14.0).into(),
                button("New (p)roject", IcedMessage::OpenPopup, green(), context.zoom).into(),
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
            )
                .padding(context.zoom * 8.0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
            .width(context.window_size.width)
            .height(context.window_size.height * 0.5)
            .into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Current tabs").size(context.zoom * 14.0).into(),
                button("New (t)ab", IcedMessage::NewTab, green(), context.zoom).into(),
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
            )
                .padding(context.zoom * 8.0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
            .width(context.window_size.width)
            .height(context.window_size.height * 0.5)
            .into(),
    ]).spacing(context.zoom * 8.0);

    Scrollable::new(c).into()
}

// <full-path> (39 minutes ago) [Launch] [Browse] [Instruction]
fn render_projects<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec((0..50).map(|_| text!("PLACEHOLDER").into()).collect()).into()
}

fn render_tabs<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut column: Vec<Element<IcedMessage>> = vec![Row::from_vec(vec![
        text!("1. ").size(context.zoom * 14.0).into(),
        circle(context.zoom * 6.0, white()),
        text!("Index").size(context.zoom * 14.0).into(),
    ]).spacing(context.zoom * 4.0).align_y(Vertical::Center).into()];

    column.extend(context.current_tabs.iter().enumerate().map(
        |(i, tab)| {
            let mut texts: Vec<Element<IcedMessage>> = vec![text!("{}", tab.title).size(context.zoom * 14.0).into()];

            if let Some(status) = &tab.status {
                texts.push(text!("{status}").size(context.zoom * 14.0).into());
            }

            if let Some(error) = &tab.error {
                texts.push(text!("{error}").color(red()).size(context.zoom * 14.0).into());
            }

            Row::from_vec(vec![
                text!("{}. ", i + 2).size(context.zoom * 14.0).into(),
                circle(context.zoom * 6.0, tab.flag),
                if texts.len() == 1 { texts.remove(0).into() } else { Column::from_vec(texts).into() },
                button("Open", IcedMessage::OpenTab { id: tab.id, index: tab.index }, green(), context.zoom).into(),
            ]).spacing(context.zoom * 4.0).align_y(Vertical::Center).into()
        }
    ));

    Column::from_vec(column).spacing(context.zoom * 8.0).into()
}
