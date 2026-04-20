use super::{black, button, gray, green, horizontal_bar, pink, red, set_bg, white};
use crate::{Error, init_working_dir};
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, MouseArea, Row, Scrollable, Sensor, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::operation::{AbsoluteOffset, scroll_to};
use iced::widget::text_editor::{Action as TextEditorAction, Content as TextEditorContent, TextEditor};
use ragit_fs::{
    basename,
    create_dir,
    current_dir,
    is_dir,
    join,
    normalize as normalize_path,
    read_dir,
    set_current_dir,
};

const HELP_MESSAGE: &str = "TODO: Write help message...";

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub cwd: String,
    pub entries: Vec<FileEntry>,
    pub has_neukgu_index: bool,
    pub window_size: Size,
    pub entry_view_id: Id,
    pub entry_view_scrolled: AbsoluteOffset,
    pub hovered_entry: Option<String>,
    pub curr_popup: Option<Popup>,

    // for `neukgu-instruction.md`
    pub long_text_editor_content: TextEditorContent,

    // for name of the new project
    pub short_text_editor_content: TextEditorContent,
}

impl IcedContext {
    pub fn close_popup(&mut self) {
        self.hovered_entry = None;
        self.curr_popup = None;
        self.long_text_editor_content = TextEditorContent::with_text("");
        self.short_text_editor_content = TextEditorContent::with_text("");
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    WindowResized(Size),
    EntryViewScrolled(AbsoluteOffset),
    HoverOnEntry(Option<String>),
    OpenPopup(Popup),
    ClosePopup,
    ChDir(String),
    Create { path: String },
    Init { path: String },
    Launch { path: String },
    EditLongText(TextEditorAction),
    EditShortText(TextEditorAction),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub has_neukgu_index: bool,

    // Error while reading this entry.
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Popup {
    Create { path: String },
    Init { path: String },
    Help,
}

pub fn boot() -> IcedContext {
    try_boot().unwrap()
}

pub fn try_boot() -> Result<IcedContext, Error> {
    let current_dir = current_dir()?;

    Ok(IcedContext {
        cwd: current_dir.to_string(),
        entries: load_entries(&current_dir)?,
        has_neukgu_index: check_neukgu_index(&current_dir)?,
        window_size: Size::new(0.0, 0.0),
        entry_view_id: Id::unique(),
        entry_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
        hovered_entry: None,
        curr_popup: None,
        long_text_editor_content: TextEditorContent::with_text(""),
        short_text_editor_content: TextEditorContent::with_text(""),
    })
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::EntryViewScrolled(o) => {
            context.entry_view_scrolled = o;
        },
        IcedMessage::HoverOnEntry(e) => {
            context.hovered_entry = e;
        },
        IcedMessage::OpenPopup(popup) => {
            context.curr_popup = Some(popup);
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::ChDir(path) => {
            context.close_popup();
            context.cwd = path.to_string();
            context.entries = load_entries(&path)?;
            context.has_neukgu_index = check_neukgu_index(&path)?;
            context.entry_view_scrolled = AbsoluteOffset { x: 0.0, y: 0.0 };
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::Create { path } => {
            let project_name = context.short_text_editor_content.text();
            let instruction = context.long_text_editor_content.text();
            let project_path = join(&path, &project_name)?;

            set_current_dir(&path)?;
            create_dir(&project_name)?;
            set_current_dir(&project_path)?;
            init_working_dir(Some(instruction), false)?;
            return Ok(Task::done(IcedMessage::Launch { path: project_path }));
        },
        IcedMessage::Init { path } => {
            let instruction = context.long_text_editor_content.text();

            set_current_dir(&path)?;
            init_working_dir(Some(instruction), false)?;
            return Ok(Task::done(IcedMessage::Launch { path }));
        },
        IcedMessage::Launch { .. } => unreachable!(),
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::EditShortText(a) => {
            context.short_text_editor_content.perform(a);
        },
        IcedMessage::Error(_) => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'a>(context: &'a IcedContext) -> Element<'a, IcedMessage> {
    let mut entries: Vec<Element<IcedMessage>> = context.entries.iter().map(
        |entry| render_entry(entry, context)
    ).collect();

    // It makes rooms for popups when there're not enough entries.
    entries.push(text!("").width(Length::Fixed(800.0)).height(Length::Fixed(800.0)).into());

    let entries_stretched = Column::from_vec(entries)
        .padding(8)
        .spacing(8);

    let mut entries_scrollable = Scrollable::new(entries_stretched).id(context.entry_view_id.clone());

    if context.curr_popup.is_none() {
        entries_scrollable = entries_scrollable.on_scroll(|v| IcedMessage::EntryViewScrolled(v.absolute_offset()));
    }

    let entries_colored = Container::new(entries_scrollable).style(|_| set_bg(black()));
    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.cwd)).padding(8).into(),
        horizontal_bar(context.window_size.width),
        render_buttons(context),
        horizontal_bar(context.window_size.width),
        entries_colored.into(),
    ]);

    let full_view_resizable = Sensor::new(full_view)
        .on_show(|s| IcedMessage::WindowResized(s))
        .on_resize(|s| IcedMessage::WindowResized(s));

    let mut full_view_stacked: Element<IcedMessage> = Container::new(full_view_resizable).into();

    if let Some(Popup::Init { path }) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_init_popup(path, context),
        ]).into();
    } else if let Some(Popup::Create { path }) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_create_popup(path, context),
        ]).into();
    } else if let Some(Popup::Help) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(Scrollable::new(text!("{HELP_MESSAGE}")).into(), context).into(),
        ]).into();
    }

    full_view_stacked.into()
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    if context.curr_popup.is_some() {
        return Container::new(text!("")).padding(8).into();
    }

    let mut buttons: Vec<Element<IcedMessage>> = vec![button("Create new", IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() }), green()).into()];

    if context.has_neukgu_index {
        buttons.push(button("Launch", IcedMessage::Launch { path: context.cwd.clone() }, green()).into());
    } else {
        buttons.push(button("Init here", IcedMessage::OpenPopup(Popup::Init { path: context.cwd.clone() }), green()).into());
    }

    buttons.push(button("Help", IcedMessage::OpenPopup(Popup::Help), pink()).into());
    Row::from_vec(buttons).padding(8).spacing(8).into()
}

fn render_entry<'e, 'c, 'm>(entry: &'e FileEntry, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let mut row = vec![];
    let char_count = entry.name.chars().count();
    let is_dir = entry.is_dir;
    let is_hovered = if let Some(e) = &context.hovered_entry { e == &entry.name } else { false };
    let truncated_name = if char_count < 27 {
        format!(
            "{}{}{}",
            entry.name,
            if entry.is_dir { "/" } else { " " },
            " ".repeat(27 - char_count),
        )
    } else {
        format!(
            "{}...{}",
            entry.name.chars().take(24).collect::<String>(),
            if is_dir { "/" } else { " " },
        )
    };
    let mut truncated_name = text!("{truncated_name}");

    if is_hovered {
        truncated_name = truncated_name.color(black());
    } else if is_dir && entry.error.is_none() {
        //
    } else {
        truncated_name = truncated_name.color(gray(0.8));
    };

    let name_bg_color = if is_hovered {
        gray(0.7)
    } else if is_dir && entry.error.is_none() {
        gray(0.3)
    } else {
        gray(0.5)
    };

    let name_container = Container::new(truncated_name).padding(8).style(
        move |_| Style {
            background: Some(Background::Color(name_bg_color)),
            border: Border {
                color: black(),
                width: 0.0,
                radius: Radius::new(8.0),
            },
            ..Style::default()
        }
    );
    let name_container: Element<IcedMessage> = if is_dir && entry.error.is_none() && context.curr_popup.is_none() {
        MouseArea::new(name_container)
            .on_enter(IcedMessage::HoverOnEntry(Some(entry.name.to_string())))
            .on_exit(IcedMessage::HoverOnEntry(None))
            .on_press(IcedMessage::ChDir(entry.path.to_string()))
            .into()
    } else {
        name_container.into()
    };

    row.push(name_container.into());

    if let Some(e) = &entry.error {
        // TODO: display {e} -> I tried iced::widget::hover, but it's not pretty...
        row.push(text!("(!)").color(red()).into());
    }

    Row::from_vec(row).align_y(Vertical::Center).into()
}

fn render_init_popup<'p, 'c>(path: &'p str, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    popup(
        Column::from_vec(vec![
            text_editor.into(),
            button("Init", IcedMessage::Init { path: path.to_string() }, green()).padding(20).into(),
        ]).spacing(20).align_x(Horizontal::Center).width(Length::Fill).into(),
        context,
    )
}

fn render_create_popup<'p, 'c>(path: &'p str, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let short_text_editor = TextEditor::new(&context.short_text_editor_content)
        .placeholder("Name of the project")
        .on_action(|action| IcedMessage::EditShortText(action));

    let long_text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    popup(
        Column::from_vec(vec![
            short_text_editor.into(),
            long_text_editor.into(),
            button("Create", IcedMessage::Create { path: path.to_string() }, green()).padding(20).into(),
        ]).spacing(20).align_x(Horizontal::Center).width(Length::Fill).into(),
        context,
    )
}

fn load_entries(path: &str) -> Result<Vec<FileEntry>, Error> {
    let mut entries = vec![FileEntry {
        name: String::from(".."),
        path: normalize_path(&join(path, "..")?)?,
        is_dir: true,
        has_neukgu_index: false,
        error: None,
    }];

    for e in read_dir(path, true)? {
        let (has_neukgu_index, error) = match check_neukgu_index(&e) {
            Ok(h) => (h, None),
            Err(e) => (false, Some(format!("{e:?}")))
        };

        entries.push(FileEntry {
            name: basename(&e)?,
            path: e.to_string(),
            is_dir: is_dir(&e),
            has_neukgu_index,
            error,
        });
    }

    Ok(entries)
}

fn check_neukgu_index(path: &str) -> Result<bool, Error> {
    Ok(is_dir(path) && {
        for child in read_dir(path, false)? {
            if basename(&child)? == ".neukgu" && is_dir(&child) {
                return Ok(true);
            }
        }

        false
    })
}

fn popup<'a, 'b>(element: Element<'a, IcedMessage>, context: &'b IcedContext) -> Element<'a, IcedMessage> {
    let mut buttons: Vec<Element<IcedMessage>> = vec![];

    // TODO: any buttons else?
    // if context.prev_popup.is_some() {
    //     buttons.push(button("Back", IcedMessage::BackPopup, blue()).into());
    // }

    // if context.copy_buffer.is_some() {
    //     buttons.push(button("Copy", IcedMessage::CopyToClipboard, blue()).into());
    // }

    buttons.push(button("Close", IcedMessage::ClosePopup, red()).into());

    Container::new(
        Container::new(Column::from_vec(vec![
            Row::from_vec(buttons).padding(8).spacing(8).into(),
            element,
        ]).width(Length::Fill)).style(
            |_| Style {
                background: Some(Background::Color(black())),
                border: Border {
                    color: white(),
                    width: 4.0,
                    radius: Radius::new(8.0),
                },
                ..Style::default()
            }
        )
        .padding(8.0)
        .width(Length::Fill)
    )
    .style(|_| set_bg(Color::from_rgba(0.0, 0.0, 0.0, 0.5)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(32.0)
    .into()
}
