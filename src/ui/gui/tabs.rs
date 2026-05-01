use super::{black, gray, set_bg, white};
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
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Space, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::{Container, Style as ContainerStyle};

pub struct IcedContext {
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
    NewTab,
    SelectTab(usize),
    CloseTab(usize),
    SelectIndex,
    None,
}

pub fn boot() -> IcedContext {
    IcedContext {
        window_size: Size::new(0.0, 0.0),
        selected_tab: Some(0),
        index: index::boot(),
        tabs: vec![tab::boot(Size::new(0.0, 0.0))],
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Index(m) => match context.selected_tab {
            Some(_) => unreachable!(),
            None => index::update(&mut context.index, m).map(|m| IcedMessage::Index(m)),
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

            Task::batch(tasks)
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("t"), true, false, false) => Task::done(IcedMessage::NewTab),
            (Key::Character("w"), true, false, false) => match context.selected_tab {
                Some(selected_tab) => Task::done(IcedMessage::CloseTab(selected_tab)),
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
                None => todo!(),
            },
        },
        IcedMessage::WindowResized(size) => {
            context.window_size = size;
            let mut tasks = vec![];

            for t in context.tabs.iter_mut() {
                tasks.push(tab::update(t, TabMessage::WindowResized(size)).map(|m| IcedMessage::Tab(m)));
            }

            Task::batch(tasks)
        },
        IcedMessage::NewTab => {
            context.selected_tab = Some(context.tabs.len());
            context.tabs.push(tab::boot(context.window_size));
            Task::none()
        },
        IcedMessage::SelectTab(i) => {
            context.selected_tab = Some(i);
            Task::none()
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
    row.push(render_tab_title("Index", white(), context.selected_tab.is_none(), IcedMessage::SelectIndex, None));

    for (i, tab) in context.tabs.iter().enumerate() {
        let (title, flag) = tab.get_title_and_flag();
        let (curr_selected, select_action) = if let Some(s) = context.selected_tab && s == i {
            (true, IcedMessage::None)
        } else {
            (false, IcedMessage::SelectTab(i))
        };
        row.push(render_tab_title(
            &title,
            flag,
            curr_selected,
            select_action,
            Some(IcedMessage::CloseTab(i)),
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
    ).on_press(IcedMessage::NewTab);

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
) -> Element<'m, IcedMessage> {
    let flag = Container::new(Space::new()).padding(6.0).style(move |_| ContainerStyle {
        background: Some(Background::Color(flag)),
        border: Border {
            color: black(),
            width: 0.0,
            radius: Radius::new(12.0),
        },
        ..ContainerStyle::default()
    });
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
        text!(" ").into()
    };

    Container::new(
        Row::from_vec(vec![
            title.into(),
            Space::new().width(Length::Fill).into(),
            close.into(),
        ])
            .align_y(Vertical::Center)
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
