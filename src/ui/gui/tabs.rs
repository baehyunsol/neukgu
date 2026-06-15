use super::{black, circle, count_chars, gray, set_bg, take_chars, white};
use super::browser::IcedMessage as BrowserMessage;
use super::index::{
    self,
    IcedContext as IndexContext,
    IcedMessage as IndexMessage,
};
use super::scratch_pad::{
    self,
    IcedContext as ScratchPadContext,
    IcedMessage as ScratchPadMessage,
    Tab as ScratchPadTab,
};
use super::tab::{
    self,
    IcedContext as TabContext,
    IcedMessage as TabMessage,
    LocalContext,
    TabId,
};
use super::worker::{JobResult, JobResultKind, Workers, init_workers};
use super::working_dir::IcedMessage as WorkingDirMessage;
use crate::{ChatId, get_neukgu_id};
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{Column, Row, Space, Stack, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::{Container, Style as ContainerStyle};
use iced::widget::operation::{focus, scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use ragit_fs::{basename, current_dir, is_dir, parent};
use std::collections::hash_map::{Entry, HashMap};

pub struct IcedContext {
    pub home_dir: String,
    pub window_size: Size,
    pub frame: usize,

    // I want to directly set env vars, but it's unsafe to do so in Rust.
    // So the api key env vars are stored here. If the same key is set in
    // the env var and this hash_map, the env var has a precedence.
    pub api_keys: HashMap<String, String>,

    // If it's `None`, the index tab is selected.
    pub selected_tab: Option<usize>,

    pub index: IndexContext,
    pub tabs: Vec<TabContext>,
    pub scratch_pad: ScratchPadContext,
    pub notes: Vec<(String, /* life_span: */ usize)>,
    pub workers: Workers,
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
            api_keys: HashMap::new(),
            selected_tab: None,
            index: IndexContext::new(&home_dir),
            tabs: vec![],
            scratch_pad: ScratchPadContext::new(),
            notes: vec![],
            workers: init_workers(8),
        }
    }

    pub fn keep_scroll(&mut self) -> Task<IcedMessage> {
        if let Some(i) = self.selected_tab {
            let id = self.tabs[i].id;
            tab::update(&mut self.tabs[i], TabMessage::Focus).map(move |message| IcedMessage::Tab { id, message })
        } else {
            index::update(&mut self.index, IndexMessage::Focus).map(IcedMessage::Index)
        }
    }

    pub fn notify(&mut self, note: String) {
        self.notes.push((note, 60));

        if self.notes.len() > 3 {
            self.notes = self.notes[1..].to_vec();
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Index(IndexMessage),
    Tab { id: TabId, message: TabMessage },
    ScratchPad(ScratchPadMessage),
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
    Chat(ChatId),
    WorkingDir(String),
}

pub fn boot(cwd: &Option<String>) -> (IcedContext, Task<IcedMessage>) {
    let initial_task = match cwd {
        Some(cwd) => match (is_dir(cwd), parent(cwd), basename(cwd)) {
            (false, Ok(dir), Ok(file)) => Task::done(IcedMessage::NewTab { tab: Tab::Browser { dir, file: Some(file) }, force_new_tab: true }),
            _ => Task::done(IcedMessage::NewTab { tab: Tab::Browser { dir: cwd.to_string(), file: None }, force_new_tab: true }),
        },
        None => Task::none(),
    };

    (IcedContext::new(), initial_task)
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    // Whenever it changes UI, some scrollbars are reset. So it calls `keep_scroll` to prevent that.
    match message {
        IcedMessage::Index(IndexMessage::OpenScratchPad { title, content }) |
        IcedMessage::Tab { id: _, message: TabMessage::OpenScratchPad { title, content } } => {
            context.scratch_pad.open_content(title, content);
            context.keep_scroll()
        },
        IcedMessage::Index(IndexMessage::Notify(note)) |
        IcedMessage::Tab { id: _, message: TabMessage::Notify(note) } |
        IcedMessage::ScratchPad(ScratchPadMessage::Notify(note)) => {
            context.notify(note);
            context.keep_scroll()
        },
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
        IcedMessage::Index(IndexMessage::BackgroundJob(w)) => {
            context.workers.push(None, w).unwrap();
            Task::none()
        },
        IcedMessage::Index(m) => match context.selected_tab {
            Some(_) => Task::none(),
            None => index::update(&mut context.index, m).map(IcedMessage::Index),
        },
        IcedMessage::Tab { id, message: TabMessage::BackgroundJob(w) } => {
            context.workers.push(Some(id), w).unwrap();
            Task::none()
        },
        IcedMessage::Tab { id: _, message: TabMessage::Browser(BrowserMessage::NewBrowser { dir, file }) | TabMessage::WorkingDir(WorkingDirMessage::OpenBrowser { dir, file }) } => {
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
        IcedMessage::ScratchPad(ScratchPadMessage::Close) => {
            context.scratch_pad.save_context();
            context.scratch_pad.tab = ScratchPadTab::Hidden;
            context.keep_scroll()
        },
        IcedMessage::ScratchPad(m) => scratch_pad::update(&mut context.scratch_pad, m).map(IcedMessage::ScratchPad),
        IcedMessage::Tick => {
            let mut tasks = vec![];
            let mut job_results_by_tab_id: HashMap<TabId, Vec<JobResult>> = HashMap::new();
            context.frame += 1;

            for (job_result, tab_id) in context.workers.poll() {
                match (job_result, tab_id) {
                    (JobResult { kind: JobResultKind::WorkerError(e), .. }, _) => {
                        context.notify(e);
                    },
                    (job_result, Some(tab_id)) => match job_results_by_tab_id.entry(tab_id) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(job_result);
                        },
                        Entry::Vacant(e) => {
                            e.insert(vec![job_result]);
                        },
                    },
                    (job_result, None) => {
                        tasks.push(index::update(&mut context.index, IndexMessage::BackgroundJobResult(job_result)).map(IcedMessage::Index));
                    },
                }
            }

            for t in context.tabs.iter_mut() {
                let id = t.id;
                tasks.push(tab::update(t, TabMessage::Tick { frame: context.frame, force_update: false }).map(move |message| IcedMessage::Tab { id, message }));

                if let Some(job_results) = job_results_by_tab_id.remove(&id) {
                    for job_result in job_results.into_iter() {
                        tasks.push(tab::update(t, TabMessage::BackgroundJobResult(job_result)).map(move |message| IcedMessage::Tab { id, message }));
                    }
                }
            }

            tasks.push(index::update(&mut context.index, IndexMessage::Tick { frame: context.frame, force_update: false }).map(IcedMessage::Index));
            context.index.current_tabs = context.tabs.iter().enumerate().map(
                |(i, t)| t.get_preview(i)
            ).collect();

            for (_, life) in context.notes.iter_mut() {
                *life -= 1;
            }

            context.notes = context.notes.drain(..).filter(
                |(_, life)| *life > 0
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
                        None => index::update(&mut context.index, IndexMessage::Focus).map(IcedMessage::Index),
                    }
                }

                else {
                    Task::none()
                }
            },
            (Key::Character("c"), true, false, true) => {
                context.scratch_pad.toggle_calendar();
                context.keep_scroll()
            },
            (Key::Character("m"), true, false, true) => {
                let mut tasks = vec![
                    focus(context.scratch_pad.text_editor_id.clone()),
                ];
                context.scratch_pad.toggle_text_editor();
                tasks.push(context.keep_scroll());

                Task::batch(tasks)
            },
            (Key::Character("p"), true, false, true) => {
                context.scratch_pad.toggle_slide_rule();
                context.keep_scroll()
            },
            (_, true, _, true) => {
                if context.scratch_pad.tab != ScratchPadTab::Hidden {
                    scratch_pad::update(&mut context.scratch_pad, ScratchPadMessage::KeyPressed { key, modifiers }).map(IcedMessage::ScratchPad)
                } else {
                    Task::none()
                }
            },
            _ => match context.selected_tab {
                Some(selected_tab) => {
                    let id = context.tabs[selected_tab].id;
                    tab::update(&mut context.tabs[selected_tab], TabMessage::KeyPressed { key, modifiers }).map(move |message| IcedMessage::Tab { id, message })
                },
                None => index::update(&mut context.index, IndexMessage::KeyPressed { key, modifiers }).map(IcedMessage::Index),
            },
        },
        IcedMessage::WindowResized(size) => {
            context.window_size = size;
            let mut tasks = vec![];
            tasks.push(index::update(&mut context.index, IndexMessage::WindowResized(size)).map(IcedMessage::Index));
            tasks.push(scratch_pad::update(&mut context.scratch_pad, ScratchPadMessage::WindowResized(size)).map(IcedMessage::ScratchPad));

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
                    Tab::Chat(id) => {
                        for (i, tab) in context.tabs.iter().enumerate() {
                            if let TabContext { local: LocalContext::Chat(c), .. } = tab {
                                if c.chat.id == *id {
                                    already_open = Some(i);
                                    break;
                                }
                            }
                        }
                    },
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
                let new_tab = TabContext::new(&context.home_dir, context.api_keys.clone(), tab, context.window_size);
                let new_tab_id = new_tab.id;
                let new_tab_index = match context.selected_tab {
                    Some(i) => i + 1,
                    None => 0,
                };
                let scroll_id = new_tab.get_scroll_id();
                context.tabs.insert(new_tab_index, new_tab);
                context.selected_tab = Some(new_tab_index);

                if let Some(scroll_id) = scroll_id {
                    Task::batch(vec![
                        scroll_to(scroll_id, AbsoluteOffset { x: 0.0, y: 0.0 }),
                        Task::done(IcedMessage::Tab { id: new_tab_id, message: TabMessage::Focus }),
                    ])
                } else {
                    Task::done(IcedMessage::Tab { id: new_tab_id, message: TabMessage::Focus })
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
                index::update(&mut context.index, IndexMessage::Focus).map(IcedMessage::Index)
            }

            else {
                Task::none()
            }
        },
        IcedMessage::SelectIndex => {
            if context.selected_tab.is_some() {
                context.selected_tab = None;
                index::update(&mut context.index, IndexMessage::Focus).map(IcedMessage::Index)
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
        index::view(&context.index).map(IcedMessage::Index)
    };

    let mut view: Element<IcedMessage> = Column::from_vec(vec![
        tabs,
        horizontal_bar,
        curr_tab,
    ]).into();

    view = Stack::from_vec(vec![
        view,
        scratch_pad::view(&context.scratch_pad).map(IcedMessage::ScratchPad).into(),
    ]).into();

    view = Stack::from_vec(vec![
        view,
        render_notes(&context.notes, &context),
    ]).into();

    view
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
        let (title, flag) = tab.get_title_and_flag();
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
    Container::new(
        Row::from_vec(row).align_y(Vertical::Center)
    ).width(context.window_size.width).padding(4.0).style(|_| set_bg(gray(0.1))).into()
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
        .width(Length::Fill)
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
            .style(move |_, status| match status {
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
                    background: Some(Background::Color(if curr_selected { gray(0.3) } else { gray(0.1) })),
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
            close.into(),
        ])
            .align_y(Vertical::Center)
            .width(width)
            .padding(4.0)
            .spacing(8.0)
    )
    .style(move |_| ContainerStyle {
        background: if curr_selected {
            Some(Background::Color(gray(0.3)))
        } else {
            Some(Background::Color(gray(0.1)))
        },
        border: Border {
            color: black(),
            width: 0.0,
            radius: Radius::new(12.0),
        },
        ..ContainerStyle::default()
    }).into()
}

fn render_notes<'c>(notes: &'c [(String, usize)], context: &'c IcedContext) -> Element<'c, IcedMessage> {
    assert!(notes.len() <= 3);

    Container::new(
        Row::from_vec(vec![
            Column::from_vec(notes.iter().map(
                |(note, life)| {
                    let alpha = match life {
                        30.. => 1.0,
                        ..30 => *life as f32 / 30.0,
                    };

                    Container::new(text!("{note}").color(Color::from_rgba(1.0, 1.0, 1.0, alpha)).size(14.0))
                        .center_x(360.0)
                        .center_y(90.0)
                        .padding(8.0)
                        .style(
                            move |_| {
                                ContainerStyle {
                                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, alpha))),
                                    border: Border {
                                        color: Color::from_rgba(1.0, 1.0, 1.0, alpha),
                                        width: 4.0,
                                        radius: Radius::new(8.0),
                                    },
                                    ..ContainerStyle::default()
                                }
                            }
                        )
                        .into()
                }
            ).collect())
                .width(context.window_size.width)
                .align_x(Horizontal::Center)
                .spacing(16.0)
                .into()
        ])
            .width(context.window_size.width)
            .height(context.window_size.height)
            .align_y(Vertical::Bottom)
    )
        .padding(context.window_size.height * 0.1)
        .width(context.window_size.width)
        .height(context.window_size.height)
        .into()
}
