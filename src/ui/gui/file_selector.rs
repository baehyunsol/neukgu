use super::{
    black,
    blue,
    button,
    count_chars,
    gray,
    green,
    red,
    set_bg,
    set_round_bg,
    take_chars,
    white,
};
use super::browser::{
    FileEntry,
    load_entries,
};
use iced::{Element, Task};
use iced::alignment::Horizontal;
use iced::border::{Border, Radius};
use iced::widget::{Column, MouseArea, Row, Scrollable, Space, text};
use iced::widget::container::{Container, Style};
use ragit_fs::parent;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub home_dir: String,
    pub cwd: String,
    pub current_entry: FileEntry,
    pub entries: Vec<FileEntry>,
    pub selected: Vec<FileEntry>,
    pub hovered: Option<String>,
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
            hovered: None,
            error,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Up,
    Chdir(String),
    Select(FileEntry),
    Hover(Option<String>),
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
            },
            Err(e) => {
                context.error = Some(format!("{e:?}"));
            },
        },
        IcedMessage::Select(entry) => {
            context.selected.push(entry);
        },
        IcedMessage::Hover(e) => {
            context.hovered = e;
        },
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
        Column::from_vec(vec![
            render_top_bar(context, width - 16.0, zoom * 36.0, zoom),
            render_file_browser(context, width - 16.0, height - zoom * 88.0, zoom),
            render_bottom_bar(context, width - 16.0, zoom * 36.0, zoom),
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

fn render_top_bar<'c>(context: &'c IcedContext, width: f32, height: f32, zoom: f32) -> Element<'c, IcedMessage> {
    Container::new(text!("{}", context.cwd).color(white()).size(zoom * 14.0))
        .width(width)
        .height(height)
        .style(|_| set_bg(gray(0.15)))
        .into()
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
        let truncated_name = if char_count < 35 {
            format!(
                "{}{}{}",
                entry.name,
                if entry.is_dir { "/" } else { " " },
                " ".repeat(35 - char_count),
            )
        } else {
            format!(
                "{}...{}",
                take_chars(&entry.name, 32),
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
            .into(),
        Scrollable::new(
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

fn render_bottom_bar<'c>(context: &'c IcedContext, width: f32, height: f32, zoom: f32) -> Element<'c, IcedMessage> {
    Container::new(Space::new())
        .width(width)
        .height(height)
        .style(|_| set_bg(gray(0.15)))
        .into()
}
