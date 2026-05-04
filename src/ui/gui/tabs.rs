use super::{black, circle, count_chars, gray, set_bg, take_chars, white};
use super::index::{
    self,
    IcedContext as IndexContext,
    IcedMessage as IndexMessage,
};
use super::tab::{
    self,
    IcedContext as TabContext,
    IcedMessage as TabMessage,
};
use super::working_dir::IcedMessage as WorkingDirMessage;
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Space, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::{Container, Style as ContainerStyle};
use iced::widget::operation::scroll_to;
use iced::widget::scrollable::AbsoluteOffset;
use ragit_fs::current_dir;

pub struct IcedContext {
    pub home_dir: String,
    pub window_size: Size,

    // If it's `None`, the index tab is selected.
    pub selected_tab: Option<usize>,

    pub index: IndexContext,
    pub tabs: Vec<TabContext>,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Index(IndexMessage),
    Tab(TabMessage),
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    NewTab(Tab),
    SelectTab(usize),

    // KillTab sends kill signal to the tab.
    // When the tab is ready to be closed, the tab will produce `Dead` signal.
    // Then, it'll produce `CloseTab` signal which actually closes the tab.
    KillTab(usize),
    CloseTab(usize),
    SelectIndex,
    None,
}

#[derive(Clone, Debug)]
pub enum Tab {
    Browser { dir: String, file: Option<String> },
    WorkingDir(String),
}

pub fn boot() -> IcedContext {
    let home_dir = match std::env::var("HOME") {
        Ok(d) => d,
        Err(_) => current_dir().unwrap(),
    };

    IcedContext {
        home_dir: home_dir.to_string(),
        window_size: Size::new(0.0, 0.0),
        selected_tab: None,
        index: index::boot(&home_dir),
        tabs: vec![],
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Index(IndexMessage::NewTab(tab)) => Task::done(IcedMessage::NewTab(tab)),
        IcedMessage::Index(IndexMessage::OpenTab { id, index }) => {
            if let Some(tab) = context.tabs.get(index) && tab.id == id {
                context.selected_tab = Some(index);
            }

            Task::none()
        },
        IcedMessage::Index(m) => match context.selected_tab {
            Some(_) => unreachable!(),
            None => index::update(&mut context.index, m).map(|m| IcedMessage::Index(m)),
        },
        IcedMessage::Tab(TabMessage::WorkingDir(WorkingDirMessage::OpenBrowser { dir, file })) => {
            Task::done(IcedMessage::NewTab(Tab::Browser { dir, file }))
        },
        IcedMessage::Tab(TabMessage::Dead) => match context.selected_tab {
            Some(selected_tab) => Task::done(IcedMessage::CloseTab(selected_tab)),
            None => unreachable!(),
        },
        IcedMessage::Tab(m) => match context.selected_tab {
            Some(selected_tab) => tab::update(&mut context.tabs[selected_tab], m).map(|m| IcedMessage::Tab(m)),
            None => unreachable!(),
        },
        IcedMessage::Tick => {
            let mut tasks = vec![];

            for t in context.tabs.iter_mut() {
                tasks.push(tab::update(t, TabMessage::Tick).map(|m| IcedMessage::Tab(m)));
            }

            tasks.push(index::update(&mut context.index, IndexMessage::Tick).map(|m| IcedMessage::Index(m)));
            context.index.current_tabs = context.tabs.iter().enumerate().map(
                |(i, t)| t.get_preview(i)
            ).collect();
            Task::batch(tasks)
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("t"), true, false, false) => Task::done(IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None })),
            (Key::Character("w"), true, false, false) => match context.selected_tab {
                Some(selected_tab) => tab::update(&mut context.tabs[selected_tab], TabMessage::Kill).map(|m| IcedMessage::Tab(m)),
                None => Task::none(),  // cannot close this
            },
            (Key::Character(n @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0")), false, true, false) => {
                let selected_tab = match n {
                    "1" => None,
                    "2" => Some(0),
                    "3" => Some(1),
                    "4" => Some(2),
                    "5" => Some(3),
                    "6" => Some(4),
                    "7" => Some(5),
                    "8" => Some(6),
                    "9" => Some(7),
                    "0" => Some(8),
                    _ => unreachable!(),
                };

                if let Some(selected_tab) = selected_tab && selected_tab >= context.tabs.len() {}

                else {
                    context.selected_tab = selected_tab;
                }

                Task::none()
            },
            _ => match context.selected_tab {
                Some(selected_tab) => tab::update(&mut context.tabs[selected_tab], TabMessage::KeyPressed { key, modifiers }).map(|m| IcedMessage::Tab(m)),
                None => index::update(&mut context.index, IndexMessage::KeyPressed { key, modifiers }).map(|m| IcedMessage::Index(m)),
            },
        },
        IcedMessage::WindowResized(size) => {
            context.window_size = size;
            let mut tasks = vec![];
            tasks.push(index::update(&mut context.index, IndexMessage::WindowResized(size)).map(|m| IcedMessage::Index(m)));

            for t in context.tabs.iter_mut() {
                tasks.push(tab::update(t, TabMessage::WindowResized(size)).map(|m| IcedMessage::Tab(m)));
            }

            Task::batch(tasks)
        },
        IcedMessage::NewTab(tab) => {
            context.selected_tab = Some(context.tabs.len());
            let new_tab = tab::boot(&context.home_dir, tab, context.window_size);
            let scroll_id = new_tab.get_scroll_id();
            context.tabs.push(new_tab);

            if let Some(scroll_id) = scroll_id {
                scroll_to(scroll_id, AbsoluteOffset { x: 0.0, y: 0.0 })
            } else {
                Task::none()
            }
        },
        IcedMessage::SelectTab(i) => {
            context.selected_tab = Some(i);
            Task::none()
        },
        IcedMessage::KillTab(i) => {
            tab::update(&mut context.tabs[i], TabMessage::Kill).map(|m| IcedMessage::Tab(m))
        },
        IcedMessage::CloseTab(i) => {
            context.tabs.remove(i);

            if let Some(selected_tab) = context.selected_tab && selected_tab >= i && selected_tab > 0 {
                *context.selected_tab.as_mut().unwrap() -= 1;
            }

            if context.tabs.is_empty() {
                context.selected_tab = None;
            }

            Task::none()
        },
        IcedMessage::SelectIndex => {
            context.selected_tab = None;
            Task::none()
        },
        IcedMessage::None => Task::none(),
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let tabs = render_tabs(context);
    let horizontal_bar = Container::new(Space::new())
        .style(|_| set_bg(white()))
        .width(Length::Fixed(context.window_size.width))
        .height(Length::Fixed(8.0))
        .into();
    let curr_tab = if let Some(selected_tab) = context.selected_tab {
        tab::view(&context.tabs[selected_tab]).map(|m| IcedMessage::Tab(m))
    } else {
        index::view(&context.index).map(|m| IcedMessage::Index(m))
    };

    Column::from_vec(vec![
        tabs,
        horizontal_bar,
        curr_tab,
    ]).into()
}

fn render_tabs<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut row: Vec<Element<'_, IcedMessage>> = vec![];
    let (mut selected_tab_width, mut unselected_tab_width) = match context.tabs.len() {
        l @ ..7 => (1.8 / (l as f32 + 2.0), 1.0 / (l as f32 + 2.0)),
        l => (0.18, 0.8 / l as f32),
    };
    selected_tab_width *= context.window_size.width;
    unselected_tab_width *= context.window_size.width;

    row.push(render_tab_title(
        "Index",
        white(),
        context.selected_tab.is_none(),
        IcedMessage::SelectIndex,
        None,
        if context.selected_tab.is_none() { selected_tab_width } else { unselected_tab_width },
    ));

    for (i, tab) in context.tabs.iter().enumerate() {
        let (title, flag) = tab.get_title_and_flag(true);
        let (curr_selected, select_action, title_width) = if let Some(s) = context.selected_tab && s == i {
            (true, IcedMessage::None, selected_tab_width)
        } else {
            (false, IcedMessage::SelectTab(i), unselected_tab_width)
        };
        row.push(render_tab_title(
            &title,
            flag,
            curr_selected,
            select_action,
            if context.tabs.len() < 4 || curr_selected { Some(IcedMessage::KillTab(i)) } else { None },
            title_width,
        ));
    }

    let new_tab = Button::new(text!("+").size(14.0)).style(
        |_, status| match status {
            ButtonStatus::Hovered => ButtonStyle {
                background: Some(Background::Color(white())),
                text_color: black(),
                border: Border {
                    color: black(),
                    width: 0.0,
                    radius: Radius::new(20.0),
                },
                ..ButtonStyle::default()
            },
            _ => ButtonStyle {
                background: None,
                text_color: white(),
                border: Border {
                    color: black(),
                    width: 0.0,
                    radius: Radius::new(20.0),
                },
                ..ButtonStyle::default()
            },
        }
    ).on_press(IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None }));

    row.push(Space::new().width(8.0).into());
    row.push(new_tab.into());
    Row::from_vec(row).align_y(Vertical::Center).padding(4.0).into()
}

fn render_tab_title<'t, 'm>(
    title: &'t str,
    flag: Color,
    curr_selected: bool,
    select_action: IcedMessage,
    close_action: Option<IcedMessage>,
    width: f32,
) -> Element<'m, IcedMessage> {
    let flag = circle(6.0, flag);
    let title_limit = (width * 0.075).round() as usize;
    let title = if title_limit < 4 {
        String::new()
    } else if count_chars(title) > title_limit {
        format!("{}...", take_chars(title, title_limit - 3))
    } else {
        title.to_string()
    };
    let title = Button::new(
        Row::from_vec(vec![
            flag.into(),
            text!("{title}").size(14.0).into(),
        ])
            .align_y(Vertical::Center)
            .spacing(4.0)
    )
        .style(|_, _| {
            ButtonStyle {
                background: None,
                text_color: white(),
                ..ButtonStyle::default()
            }
        })
        .on_press(select_action);

    let close: Element<IcedMessage> = if let Some(action) = close_action {
        Button::new(text!("X").size(11.0))
            .style(|_, status| match status {
                ButtonStatus::Hovered => ButtonStyle {
                    background: Some(Background::Color(white())),
                    text_color: black(),
                    border: Border {
                        color: black(),
                        width: 0.0,
                        radius: Radius::new(20.0),
                    },
                    ..ButtonStyle::default()
                },
                _ => ButtonStyle {
                    background: Some(Background::Color(gray(0.3))),
                    text_color: white(),
                    border: Border {
                        color: black(),
                        width: 0.0,
                        radius: Radius::new(20.0),
                    },
                    ..ButtonStyle::default()
                },
            })
            .on_press(action)
            .into()
    } else {
        Space::new().into()
    };

    Container::new(
        Row::from_vec(vec![
            title.into(),
            Space::new().width(Length::Fill).into(),
            close.into(),
        ])
            .align_y(Vertical::Center)
            .width(width)
            .padding(4.0)
            .spacing(8.0)
    )
    .style(move |_| ContainerStyle {
        background: if curr_selected { Some(Background::Color(gray(0.3))) } else { None },
        border: Border {
            color: black(),
            width: 0.0,
            radius: Radius::new(12.0),
        },
        ..ContainerStyle::default()
    }).into()
}
