use super::{black, button, circle, gray, green, red, white};
use super::tab::{TabId, TabPreview};
use super::tabs::Tab;
use chrono::Local;
use crate::{Project, clean_global_index_dir, get_global_index_dir, load_all_indexes, prettify_time};
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Scrollable, text};
use iced::widget::container::{Container, Style};

pub struct IcedContext {
    pub home_dir: String,
    pub window_size: Size,
    pub recent_projects: Vec<Project>,
    pub current_tabs: Vec<TabPreview>,
    pub curr_popup: Option<(/* TODO */)>,
    pub zoom: f32,
    pub now: String,
    pub global_index_dir: String,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    NewTab(Tab),
    OpenTab { id: TabId, index: usize },
    OpenPopup(Popup),
}

#[derive(Clone, Debug)]
pub enum Popup {
    NewProject,
    Instruction(String),
}

pub fn boot(home_dir: &str) -> IcedContext {
    let global_index_dir = get_global_index_dir().unwrap();
    clean_global_index_dir(&global_index_dir).unwrap();

    IcedContext {
        home_dir: home_dir.to_string(),
        window_size: Size::new(0.0, 0.0),
        recent_projects: vec![],
        current_tabs: vec![],
        curr_popup: None,
        zoom: 1.0,
        now: Local::now().to_rfc2822(),
        global_index_dir,
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Tick => {
            context.now = Local::now().to_rfc2822();
            context.recent_projects = load_all_indexes(&context.global_index_dir);

            // let's just assume that there's no overflow!
            context.recent_projects.sort_by_key(|p| -p.updated_at);
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("p"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Task::done(IcedMessage::OpenPopup(Popup::NewProject));
                }
            },
            (Key::Character("t"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Task::done(IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None }));
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
        IcedMessage::NewTab(_) => unreachable!(),
        IcedMessage::OpenTab { .. } => unreachable!(),
        IcedMessage::OpenPopup(popup) => todo!(),
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let c = Column::from_vec(vec![
        text!("{}", context.now).size(context.zoom * 14.0).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Recent projects").size(context.zoom * 14.0).into(),
                button("New (p)roject", IcedMessage::OpenPopup(Popup::NewProject), green(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into(),
            Container::new(Scrollable::new(render_projects(context)).width(Length::Fill)).style(
                |_| Style {
                    background: Some(Background::Color(black())),
                    border: Border {
                        color: white(),
                        width: 0.0,
                        radius: Radius::new(context.zoom * 8.0),
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
                button("New (t)ab", IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None }), green(), context.zoom).into(),
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

fn render_projects<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let now = Local::now().timestamp_millis();
    let column: Vec<Element<IcedMessage>> = context.recent_projects.iter().map(
        |project| {
            let elapsed = match now - project.updated_at {
                ..0 => String::from("past"),
                ..10_000 => String::from("now"),
                d => format!("{} ago", prettify_time(d as u64)),
            };

            Container::new(
                Column::from_vec(vec![
                    text!("{} ({elapsed})", project.working_dir).size(context.zoom * 14.0).width(context.window_size.width).into(),
                    Row::from_vec(vec![
                        button("Launch", IcedMessage::NewTab(Tab::WorkingDir(project.working_dir.to_string())), green(), context.zoom).into(),
                        button("Browse", IcedMessage::NewTab(Tab::Browser { dir: project.working_dir.to_string(), file: None }), green(), context.zoom).into(),
                        button("Instruction", IcedMessage::OpenPopup(Popup::Instruction(project.working_dir.to_string())), green(), context.zoom).into(),
                    ]).spacing(context.zoom * 4.0).into(),
                ]).spacing(context.zoom * 4.0)
            ).style(
                |_| Style {
                    background: Some(Background::Color(gray(0.3))),
                    border: Border {
                        color: white(),
                        width: 0.0,
                        radius: Radius::new(context.zoom * 8.0),
                    },
                    ..Style::default()
                }
            ).padding(context.zoom * 8.0).into()
        }
    ).collect();
    Column::from_vec(column).spacing(context.zoom * 8.0).into()
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
