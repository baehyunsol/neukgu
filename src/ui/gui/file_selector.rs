use super::{
    black,
    blue,
    button,
    count_chars,
    gray,
    green,
    red,
    red_transparent,
    set_bg,
    set_round_bg,
    take_chars,
    white,
};
use super::browser::{
    FileEntry,
    load_entries,
};
use crate::Error;
use iced::{Background, Element, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, MouseArea, Row, Space, text};
use iced::widget::button::{
    Button,
    Status as ButtonStatus,
    Style as ButtonStyle,
};
use iced::widget::container::{Container, Style};
use iced::widget::operation::snap_to;
use iced::widget::scrollable::{RelativeOffset, Scrollable};
use ragit_fs::{
    copy_dir,
    copy_file,
    exists,
    extension,
    file_name,
    join,
    parent,
    set_extension,
};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub home_dir: String,
    pub cwd: String,
    pub current_entry: FileEntry,
    pub entries: Vec<FileEntry>,
    pub selected: Vec<FileEntry>,
    pub selected_set: HashSet<String>,
    pub hovered: Option<String>,
    pub entry_scroll_id: Id,
    pub error: Option<String>,
}

impl IcedContext {
    pub fn new(home_dir: String) -> IcedContext {
        let cwd = home_dir.to_string();
        let current_entry = FileEntry::from_cwd(&cwd);
        let (entries, error) = match load_entries(&home_dir) {
            Ok(entries) => (entries, None),
            Err(e) => (vec![], Some(format!("{e:?}"))),
        };

        IcedContext {
            home_dir,
            cwd,
            current_entry,
            entries,
            selected: vec![],
            selected_set: HashSet::new(),
            hovered: None,
            entry_scroll_id: Id::unique(),
            error,
        }
    }

    pub fn copy_selected_files(&self, dir: &str) -> Result<(), Error> {
        for entry in self.selected.iter() {
            let mut dst = join(dir, &entry.name)?;

            if exists(&dst) {
                for i in 1..999 {
                    let new_name = format!("{}-{i}", &file_name(&entry.name)?);
                    let new_name = match &extension(&entry.name)? {
                        Some(ext) => set_extension(&new_name, &ext)?,
                        None => new_name,
                    };
                    dst = join(dir, &new_name)?;

                    if !exists(&dst) {
                        break;
                    }
                }
            }

            if entry.is_dir {
                copy_dir(&entry.path, &dst)?;
            }

            else {
                copy_file(&entry.path, &dst)?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Up,
    Chdir(String),
    Select(FileEntry),
    Unselect(String),
    Hover(Option<String>),
    Notify(String),
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Up => match parent(&context.cwd) {
            Ok(cwd) => return Task::done(IcedMessage::Chdir(cwd)),
            Err(e) => {
                context.error = Some(format!("{e:?}"));
            },
        },
        IcedMessage::Chdir(path) => match load_entries(&path) {
            Ok(entries) => {
                context.entries = entries;
                context.cwd = path.to_string();
                context.current_entry = FileEntry::from_cwd(&path);
                context.hovered = None;
                context.error = None;
                return snap_to(context.entry_scroll_id.clone(), RelativeOffset { x: 0.0, y: 0.0 });
            },
            Err(e) => {
                context.error = Some(format!("{e:?}"));
            },
        },
        IcedMessage::Select(entry) => {
            if !context.selected_set.contains(&entry.path) {
                context.selected_set.insert(entry.path.to_string());
                context.selected.push(entry);
            }

            else {
                return Task::done(IcedMessage::Notify(String::from("It's already selected.")));
            }
        },
        IcedMessage::Unselect(path) => {
            context.selected_set.remove(&path);
            context.selected = context.selected.drain(..).filter(
                |entry| entry.path != path
            ).collect();
        },
        IcedMessage::Hover(e) => {
            context.hovered = e;
        },
        IcedMessage::Notify(_) => unreachable!(),
    }

    Task::none()
}

pub fn view<'c>(
    context: &'c IcedContext,
    border: bool,
    width: f32,
    height: f32,
    zoom: f32,
) -> Element<'c, IcedMessage> {
    let content: Element<IcedMessage> = if let Some(e) = &context.error {
        Scrollable::new(Column::from_vec(vec![
            Space::new().width(width).into(),
            text!("{e}").color(red()).size(zoom * 16.0).into(),
            button("Okay", IcedMessage::Chdir(context.home_dir.to_string()), green(), zoom).into(),
        ])
            .align_x(Horizontal::Center)
            .padding(zoom * 8.0)
            .spacing(zoom * 20.0),
        )
            .into()
    }

    else {
        let bottom_bar: Element<IcedMessage> = if context.selected.is_empty() {
            text!("Nothing's selected").color(white()).size(zoom * 14.0).into()
        } else {
            render_selected_files(&context.selected, width - 16.0, zoom)
        };

        Column::from_vec(vec![
            Container::new(text!("{}", context.cwd).color(white()).size(zoom * 14.0))
                .width(width - 16.0)
                .height(zoom * 36.0)
                .style(|_| set_bg(gray(0.15)))
                .into(),
            render_file_browser(context, width - 16.0, height - zoom * 88.0, zoom),
            Container::new(bottom_bar)
                .width(width - 16.0)
                .height(zoom * 36.0)
                .style(|_| set_bg(gray(0.15)))
                .into(),
        ])
            .padding(zoom * 8.0)
            .into()
    };

    if border {
        Container::new(content)
            .width(width)
            .height(height)
            .style(move |_| Style {
                background: None,
                border: Border {
                    color: white(),
                    width: zoom * 4.0,
                    radius: Radius::new(0),
                },
                ..Style::default()
            })
            .into()
    } else {
        content
    }
}

fn render_file_browser<'c>(context: &'c IcedContext, width: f32, height: f32, zoom: f32) -> Element<'c, IcedMessage> {
    let mut entries: Vec<Element<IcedMessage>> = vec![
        Row::from_vec(vec![
            button("Up", IcedMessage::Up, blue(), zoom).into(),
            button("Select this dir", IcedMessage::Select(context.current_entry.clone()), green(), zoom).into(),
        ])
            .spacing(zoom * 4.0)
            .into(),
    ];

    for entry in context.entries.iter() {
        let char_count = count_chars(&entry.name);
        let is_hovered = if let Some(e) = &context.hovered { e == &entry.path } else { false };
        let truncated_name = if char_count < 39 {
            format!(
                "{}{}{}",
                entry.name,
                if entry.is_dir { "/" } else { " " },
                " ".repeat(39 - char_count),
            )
        } else {
            format!(
                "{}...{}",
                take_chars(&entry.name, 36),
                if entry.is_dir { "/" } else { " " },
            )
        };
        let bg_color = if is_hovered {
            gray(0.7)
        } else if entry.error.is_none() {
            gray(0.3)
        } else {
            gray(0.5)
        };
        let mut truncated_name = text!("{truncated_name}").color(white()).size(zoom * 12.0);

        if is_hovered {
            truncated_name = truncated_name.color(black());
        } else if entry.error.is_none() {
            //
        } else {
            truncated_name = truncated_name.color(gray(0.8));
        };

        let mut m = MouseArea::new(
            Container::new(truncated_name)
                .padding(zoom * 4.0)
                .style(move |_| set_round_bg(bg_color, zoom))
        );

        if entry.error.is_none() {
            m = m
                .on_enter(IcedMessage::Hover(Some(entry.path.to_string())))
                .on_exit(IcedMessage::Hover(None))
                .on_press(if entry.is_dir {
                    IcedMessage::Chdir(entry.path.to_string())
                } else {
                    IcedMessage::Select(entry.clone())
                });
        }

        entries.push(m.into());
    }

    Row::from_vec(vec![
        Scrollable::new(
            Container::new(
                Column::from_vec(entries)
                    .width(width * 0.7)
                    .spacing(zoom * 4.0)
            )
                .width(width * 0.7)
                .style(|_| set_bg(black())),
        )
            .id(context.entry_scroll_id.clone())
            .into(),
        Scrollable::new(
            // TODO: render this side
            // 1. Preview text files (first few hundred bytes)
            // 2. Preview image (make a small thumbnail)
            // 3. ... or what else?
            // -> In order to do this, I have to connect this to background workers.
            Container::new(Space::new())
                .width(width * 0.3)
                .style(|_| set_bg(gray(0.2))),
        )
            .into(),
    ])
        .width(width)
        .height(height)
        .into()
}

pub fn render_selected_files<'c>(selected: &'c [FileEntry], width: f32, zoom: f32) -> Element<'c, IcedMessage> {
    Scrollable::new(
        Row::from_vec(selected.iter().map(
            move |selection| {
                Container::new(
                    Row::from_vec(vec![
                        text!("{}", selection.name).color(black()).size(zoom * 12.0).into(),
                        Button::new(text!("X").color(white()).size(zoom * 12.0))
                            .padding(zoom * 4.0)
                            .style(move |_, status| {
                                let bg_color = match status {
                                    ButtonStatus::Hovered => red_transparent(),
                                    _ => red(),
                                };

                                ButtonStyle {
                                    background: Some(Background::Color(bg_color)),
                                    text_color: white(),
                                    border: Border {
                                        color: white(),
                                        width: 0.0,
                                        radius: Radius::new(zoom * 4.0),
                                    },
                                    ..ButtonStyle::default()
                                }
                            })
                            .on_press(IcedMessage::Unselect(selection.path.to_string()))
                            .into(),
                    ])
                        .align_y(Vertical::Center)
                        .spacing(zoom * 4.0)
                )
                    .padding(zoom * 4.0)
                    .style(move |_| set_round_bg(gray(0.8), zoom))
                    .into()
            }
        ).collect())
            .align_y(Vertical::Center)
            .padding(zoom * 4.0)
            .spacing(zoom * 4.0)
    )
        .width(width)
        .horizontal()
        .anchor_right()
        .into()

}
