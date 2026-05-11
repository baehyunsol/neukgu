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
    LocalContext,
    TabId,
};
use super::working_dir::IcedMessage as WorkingDirMessage;
use crate::get_neukgu_id;
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Space, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::{Container, Style as ContainerStyle};
use iced::widget::operation::scroll_to;
use iced::widget::scrollable::AbsoluteOffset;
use ragit_fs::{basename, current_dir};

pub struct IcedContext {
    pub home_dir: String,
    pub window_size: Size,
    pub frame: usize,

    // If it's `None`, the index tab is selected.
    pub selected_tab: Option<usize>,

    pub index: IndexContext,
    pub tabs: Vec<TabContext>,
}

impl IcedContext {
    pub fn new() -> IcedContext {
        let home_dir = match std::env::var("HOME") {
            Ok(d) => d,
            Err(_) => current_dir().unwrap(),
        };

        IcedContext {
            home_dir: home_dir.to_string(),
            window_size: Size::new(0.0, 0.0),
            frame: 0,
            selected_tab: None,
            index: IndexContext::new(&home_dir),
            tabs: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Index(IndexMessage),
    Tab { id: TabId, message: TabMessage },
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    NewTab { tab: Tab, force_new_tab: bool },
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
    Chat,
    WorkingDir(String),
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Index(IndexMessage::NewTab { tab, force_new_tab }) => Task::done(IcedMessage::NewTab { tab, force_new_tab }),
        IcedMessage::Index(IndexMessage::OpenTab { id, index }) => {
            if let Some(tab) = context.tabs.get(index) && tab.id == id {
                let id = context.tabs[index].id;
                context.selected_tab = Some(index);
                tab::update(&mut context.tabs[index], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
            }

            else {
                Task::none()
            }
        },
        IcedMessage::Index(m) => match context.selected_tab {
            Some(_) => Task::none(),
            None => index::update(&mut context.index, m).map(|m| IcedMessage::Index(m)),
        },
        IcedMessage::Tab { id: _, message: TabMessage::WorkingDir(WorkingDirMessage::OpenBrowser { dir, file }) } => {
            Task::done(IcedMessage::NewTab { tab: Tab::Browser { dir, file }, force_new_tab: false })
        },
        IcedMessage::Tab { id, message: TabMessage::Dead } => {
            for (i, tab) in context.tabs.iter().enumerate() {
                if tab.id == id {
                    return Task::done(IcedMessage::CloseTab(i));
                }
            }

            // perhaps it's already dead?
            Task::none()
        },
        IcedMessage::Tab { id, message } => {
            for tab in context.tabs.iter_mut() {
                if tab.id == id {
                    return tab::update(tab, message).map(move |message| IcedMessage::Tab { id, message });
                }
            }

            // perhaps it's already dead?
            Task::none()
        },
        IcedMessage::Tick => {
            let mut tasks = vec![];
            context.frame += 1;

            for t in context.tabs.iter_mut() {
                let id = t.id;
                tasks.push(tab::update(t, TabMessage::Tick { frame: context.frame, force_update: false }).map(move |message| IcedMessage::Tab { id, message }));
            }

            tasks.push(index::update(&mut context.index, IndexMessage::Tick { frame: context.frame, force_update: false }).map(|m| IcedMessage::Index(m)));
            context.index.current_tabs = context.tabs.iter().enumerate().map(
                |(i, t)| t.get_preview(i)
            ).collect();
            Task::batch(tasks)
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Character("t"), true, false, false) => Task::done(IcedMessage::NewTab {
                tab: Tab::Browser { dir: context.home_dir.to_string(), file: None },
                force_new_tab: true,
            }),
            (Key::Character("w"), true, false, false) => match context.selected_tab {
                Some(selected_tab) => {
                    let id = context.tabs[selected_tab].id;
                    tab::update(&mut context.tabs[selected_tab], TabMessage::Kill).map(move |message| IcedMessage::Tab { id, message })
                },
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

                if let Some(selected_tab) = selected_tab && selected_tab >= context.tabs.len() {
                    Task::none()
                }

                else if context.selected_tab != selected_tab {
                    context.selected_tab = selected_tab;

                    match context.selected_tab {
                        Some(i) => {
                            let id = context.tabs[i].id;
                            tab::update(&mut context.tabs[i], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
                        },
                        None => index::update(&mut context.index, IndexMessage::Focus).map(|m| IcedMessage::Index(m)),
                    }
                }

                else {
                    Task::none()
                }
            },
            _ => match context.selected_tab {
                Some(selected_tab) => {
                    let id = context.tabs[selected_tab].id;
                    tab::update(&mut context.tabs[selected_tab], TabMessage::KeyPressed { key, modifiers }).map(move |message| IcedMessage::Tab { id, message })
                },
                None => index::update(&mut context.index, IndexMessage::KeyPressed { key, modifiers }).map(|m| IcedMessage::Index(m)),
            },
        },
        IcedMessage::WindowResized(size) => {
            context.window_size = size;
            let mut tasks = vec![];
            tasks.push(index::update(&mut context.index, IndexMessage::WindowResized(size)).map(|m| IcedMessage::Index(m)));

            for t in context.tabs.iter_mut() {
                let id = t.id;
                tasks.push(tab::update(t, TabMessage::WindowResized(size)).map(move |message| IcedMessage::Tab { id, message }));
            }

            Task::batch(tasks)
        },
        IcedMessage::NewTab { tab, force_new_tab } => {
            let mut already_open = None;

            if !force_new_tab {
                match &tab {
                    Tab::WorkingDir(w) => match get_neukgu_id(w) {
                        Ok(id) => {
                            for (i, tab) in context.tabs.iter().enumerate() {
                                if let TabContext { local: LocalContext::WorkingDir(w), .. } = tab && w.fe_context.neukgu_id == id {
                                    already_open = Some(i);
                                    break;
                                }
                            }
                        },

                        // `TabContext::new` will encounter the same error and will handle that
                        Err(_) => {},
                    },
                    Tab::Browser { dir, file } => {
                        let file = file.as_ref().map(|file| basename(file).unwrap_or(String::new()));

                        for (i, tab) in context.tabs.iter().enumerate() {
                            if let TabContext { local: LocalContext::Browser(b), .. } = tab {
                                if let Ok((tab_dir, tab_file)) = b.get_open_dir_and_file() {
                                    if dir == &tab_dir && file == tab_file {
                                        already_open = Some(i);
                                        break;
                                    }
                                }
                            }
                        }
                    },
                    Tab::Chat => todo!(),
                }
            }

            if let Some(index) = already_open {
                if context.selected_tab != Some(index) {
                    let id = context.tabs[index].id;
                    context.selected_tab = Some(index);
                    tab::update(&mut context.tabs[index], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
                }

                else {
                    Task::none()
                }
            }

            else {
                let new_tab = TabContext::new(&context.home_dir, tab, context.window_size);
                let new_tab_index = match context.selected_tab {
                    Some(i) => i + 1,
                    None => 0,
                };
                let scroll_id = new_tab.get_scroll_id();
                context.tabs.insert(new_tab_index, new_tab);
                context.selected_tab = Some(new_tab_index);

                if let Some(scroll_id) = scroll_id {
                    scroll_to(scroll_id, AbsoluteOffset { x: 0.0, y: 0.0 })
                } else {
                    Task::none()
                }
            }
        },
        IcedMessage::SelectTab(i) => {
            if Some(i) != context.selected_tab {
                let id = context.tabs[i].id;
                context.selected_tab = Some(i);
                tab::update(&mut context.tabs[i], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
            }

            else {
                Task::none()
            }
        },
        IcedMessage::KillTab(i) => {
            let id = context.tabs[i].id;

            // It'll open a popup, so, in order for the user to see the popup, the tab has to be selected.
            if let LocalContext::WorkingDir(_) = &context.tabs[i].local {
                context.selected_tab = Some(i);
            }

            tab::update(&mut context.tabs[i], TabMessage::Kill).map(move |message| IcedMessage::Tab { id, message })
        },
        IcedMessage::CloseTab(i) => {
            context.tabs.remove(i);

            if let Some(selected_tab) = context.selected_tab && selected_tab >= i && selected_tab > 0 {
                let id = context.tabs[selected_tab - 1].id;
                context.selected_tab = Some(selected_tab - 1);
                tab::update(&mut context.tabs[selected_tab - 1], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
            }

            else if context.tabs.is_empty() {
                context.selected_tab = None;
                index::update(&mut context.index, IndexMessage::Focus).map(|m| IcedMessage::Index(m))
            }

            else {
                Task::none()
            }
        },
        IcedMessage::SelectIndex => {
            if context.selected_tab.is_some() {
                context.selected_tab = None;
                index::update(&mut context.index, IndexMessage::Focus).map(|m| IcedMessage::Index(m))
            }

            else {
                Task::none()
            }
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
        let selected_tab = &context.tabs[selected_tab];
        let selected_tab_id = selected_tab.id;
        tab::view(&selected_tab).map(move |message| IcedMessage::Tab { id: selected_tab_id, message })
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
    ).on_press(IcedMessage::NewTab { tab: Tab::Browser { dir: context.home_dir.to_string(), file: None }, force_new_tab: true });

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
    // 12 for flag, 4 for spacing between flag and title, 8 for spacing between title, 8 for padding and close button and 24 for close button
    let title_max_width = width - 32.0 - if close_action.is_some() { 24.0 } else { 0.0 };
    let title_limit = ((title_max_width * 0.09).round() as i64).max(0) as usize;
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
                        radius: Radius::new(12.0),
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
