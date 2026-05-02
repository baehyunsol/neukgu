use super::{black, blue, button, disabled_button, gray, green, pink, red, set_bg, white};
use crate::{Error, Model, init_working_dir, prettify_bytes, validate_project_name};
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, Id, MouseArea, Radio, Row, Scrollable, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{
    Handle as ImageHandle,
    Viewer as ImageViewer,
};
use iced::widget::operation::{AbsoluteOffset, scroll_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    TextEditor,
};
use ragit_fs::{
    basename,
    create_dir,
    extension,
    file_size,
    is_dir,
    join,
    normalize as normalize_path,
    parent,
    read_bytes,
    read_bytes_offset,
    read_dir,
    remove_dir_all,
    remove_file,
};
use std::sync::Arc;

const HELP_MESSAGE: &str = r#"
There are multiple ways to work with neukgu.

1. Create a new project and make neukgu work in the new directory.
   In order to do this, click the "Create new" button.

2. You already have a working directory, and you want neukgu to work
   in the existing directory.
   In order to do this, go to the directory and click the "Init here" button.
   If neukgu is already working in the directory, you'll see a "Launch" button.
"#;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub cwd: String,
    pub entries: Vec<FileEntry>,
    pub has_neukgu_index: bool,
    pub window_size: Size,
    pub entry_view_id: Id,
    pub entry_view_scrolled: AbsoluteOffset,

    // hovered_entry: mouse
    // selected_entry: arrow keys
    pub hovered_entry: Option<String>,
    pub selected_entry: Option<usize>,

    pub curr_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub image_buffer: Option<ImageHandle>,
    pub syntax_highlight: Option<String>,
    pub long_preview: Option<(String, usize, String)>,
    pub popup_title: Option<String>,
    pub zoom: f32,

    // for `neukgu-instruction.md`
    pub long_text_editor_content: TextEditorContent,

    // for name of the new project
    pub short_text_editor_content: TextEditorContent,

    pub selected_model: Model,
}

impl IcedContext {
    // It returns a scroll-offset of the entry view.
    pub fn select_entry(&mut self, offset: i32) -> f32 {
        let new_selection = (self.selected_entry.map(|i| i as i32).unwrap_or(-1) + offset).min(self.entries.len() as i32 - 1).max(0) as usize;
        self.selected_entry = Some(new_selection);
        self.zoom * (new_selection.max(3) - 3) as f32 * 42.3
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::Create { .. } => {},
            Popup::Init { .. } => {},
            Popup::EntryError(e) => {
                self.copy_buffer = Some(e.to_string());
                self.set_text_editor_content(e.to_string());
                self.syntax_highlight = None;
            },
            Popup::Preview { path } => {
                let mut is_binary = false;
                let file_size = file_size(&path)? as usize;
                let content = if file_size > 33554432 {
                    is_binary = true;
                    let pre = read_bytes_offset(&path, 0, 16384)?;
                    let mut post_offset = file_size - 16384;
                    post_offset -= post_offset % 32;
                    let post = read_bytes_offset(&path, post_offset as u64, file_size as u64)?;
                    vec![
                        dump_hex(&pre, 0),
                        dump_hex(&post, post_offset),
                    ].concat()
                } else {
                    match String::from_utf8(read_bytes(&path)?) {
                        Ok(s) => {
                            self.syntax_highlight = extension(&path)?;
                            s
                        },
                        Err(e) => match image::load_from_memory(e.as_bytes()) {
                            Ok(_) => {
                                self.image_buffer = Some(ImageHandle::from_bytes(e.as_bytes().to_vec()));
                                String::new()
                            },
                            _ => {
                                is_binary = true;
                                dump_hex(e.as_bytes(), 0)
                            },
                        },
                    }
                };

                let preview = if content.chars().count() > 32768 {
                    // hex_dump's line is 84 characters, so it shows 4KiB if the file is binary
                    let pre = content.chars().take(10751).collect::<String>();
                    let post = content.chars().collect::<Vec<_>>().into_iter().rev().take(10752).rev().collect::<String>();
                    let trunc = if is_binary {
                        file_size - 4096
                    } else {
                        content.len() - pre.len() - post.len()
                    };

                    self.syntax_highlight = None;
                    self.long_preview = Some((pre, trunc, post));
                    String::new()
                } else {
                    content.to_string()
                };

                self.popup_title = Some(path);
                self.set_text_editor_content(preview.to_string());
                self.copy_buffer = Some(content.to_string());
            },
            Popup::AskDelete { .. } => {},
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_text_editor_content(HELP_MESSAGE.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
        self.hovered_entry = None;
        self.curr_popup = None;
        self.copy_buffer = None;
        self.image_buffer = None;
        self.long_preview = None;
        self.popup_title = None;
        self.long_text_editor_content = TextEditorContent::with_text("");
        self.short_text_editor_content = TextEditorContent::with_text("");
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.long_text_editor_content.perform(TextEditorAction::SelectAll);
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    EntryViewScrolled(AbsoluteOffset),
    HoverOnEntry(Option<String>),
    OpenPopup(Popup),
    ClosePopup,
    CopyToClipboard,
    ChDir(String),
    DeleteFile(String),
    DeleteDirectory(String),
    Create { path: String },
    Init { path: String },
    Launch { path: String },
    EditLongText(TextEditorAction),
    EditShortText(TextEditorAction),
    SelectModel(Model),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub has_neukgu_index: bool,
    pub size: Option<u64>,

    // Error while reading this entry.
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Popup {
    Create { path: String },
    Init { path: String },
    EntryError(String),
    Preview { path: String },
    AskDelete { is_dir: bool, path: String },
    Help,
}

pub fn try_boot(window_size: Size, cwd: &str, file: &Option<String>) -> Result<IcedContext, Error> {
    let file = match file {
        Some(file) => Some(basename(file)?),
        None => None,
    };

    let mut context = IcedContext {
        cwd: cwd.to_string(),
        entries: load_entries(cwd)?,
        has_neukgu_index: check_neukgu_index(cwd)?,
        window_size,
        entry_view_id: Id::unique(),
        entry_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
        hovered_entry: None,
        selected_entry: None,
        curr_popup: None,
        copy_buffer: None,
        image_buffer: None,
        syntax_highlight: None,
        long_preview: None,
        popup_title: None,
        zoom: 1.0,
        long_text_editor_content: TextEditorContent::new(),
        short_text_editor_content: TextEditorContent::new(),
        selected_model: Model::default(),
    };

    if let Some(file) = &file {
        context.open_popup(Popup::Preview { path: join(cwd, file)? })?;
    }

    Ok(context)
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick => {
            if context.curr_popup.is_none() {
                context.entries = load_entries(&context.cwd)?;
                context.has_neukgu_index = check_neukgu_index(&context.cwd)?;
            }
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Escape), false, false, false) => {
                return Ok(Task::done(IcedMessage::ClosePopup));
            },
            (Key::Named(NamedKey::ArrowUp), false, true, false) => {
                return Ok(Task::done(IcedMessage::ChDir(parent(&context.cwd)?)));
            },
            (Key::Named(NamedKey::ArrowUp), ctrl, false, false) => {
                if context.curr_popup.is_none() {
                    let scroll_index = context.select_entry(if ctrl { -10 } else { -1 });
                    return Ok(scroll_to(context.entry_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }
            },
            (Key::Named(NamedKey::ArrowDown), ctrl, false, false) => {
                if context.curr_popup.is_none() {
                    let scroll_index = context.select_entry(if ctrl { 10 } else { 1 });
                    return Ok(scroll_to(context.entry_view_id.clone(), AbsoluteOffset { x: 0.0, y: scroll_index }));
                }
            },
            (Key::Named(NamedKey::Enter), false, false, false) => {
                if context.curr_popup.is_none() && let Some(i) = context.selected_entry {
                    match context.entries.get(i) {
                        Some(entry) if entry.is_dir => {
                            return Ok(Task::done(IcedMessage::ChDir(entry.path.to_string())));
                        },
                        Some(entry) => {
                            return Ok(Task::done(IcedMessage::OpenPopup(Popup::Preview { path: entry.path.to_string() })));
                        },
                        None => {},
                    }
                }
            },
            (Key::Named(NamedKey::Delete), false, false, false) => {
                if context.curr_popup.is_none() && let Some(i) = context.selected_entry {
                    match context.entries.get(i) {
                        Some(entry) => {
                            return Ok(Task::done(IcedMessage::OpenPopup(Popup::AskDelete { is_dir: entry.is_dir, path: entry.path.to_string() })));
                        },
                        None => {},
                    }
                }
            },
            (Key::Character("c"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() })));
                }
            },
            (Key::Character("h"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Help)));
                }
            },
            (Key::Character("i"), false, false, false) => {
                if context.curr_popup.is_none() && !context.has_neukgu_index {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Init { path: context.cwd.clone() })));
                }
            },
            (Key::Character("l"), false, false, false) => {
                if context.curr_popup.is_none() && context.has_neukgu_index {
                    return Ok(Task::done(IcedMessage::Launch { path: context.cwd.clone() }));
                }
            },
            (Key::Character("y"), false, false, false) => {
                if let Some(Popup::AskDelete { is_dir, path }) = &context.curr_popup {
                    if *is_dir {
                        return Ok(Task::done(IcedMessage::DeleteDirectory(path.to_string())));
                    }

                    else {
                        return Ok(Task::done(IcedMessage::DeleteFile(path.to_string())));
                    }
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
        IcedMessage::EntryViewScrolled(o) => {
            context.entry_view_scrolled = o;
        },
        IcedMessage::HoverOnEntry(e) => {
            context.hovered_entry = e;
        },
        IcedMessage::OpenPopup(popup) => {
            context.open_popup(popup)?;
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::CopyToClipboard => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::ChDir(path) => {
            context.close_popup();
            context.cwd = path.to_string();
            context.entries = load_entries(&path)?;
            context.has_neukgu_index = check_neukgu_index(&path)?;
            context.entry_view_scrolled = AbsoluteOffset { x: 0.0, y: 0.0 };
            context.selected_entry = None;
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::DeleteFile(path) => {
            context.close_popup();
            remove_file(&path)?;
            context.entries = load_entries(&context.cwd)?;
        },
        IcedMessage::DeleteDirectory(path) => {
            context.close_popup();
            remove_dir_all(&path)?;
            context.entries = load_entries(&context.cwd)?;
        },
        IcedMessage::Create { path } => {
            let project_name = context.short_text_editor_content.text();
            validate_project_name(&project_name)?;
            let instruction = context.long_text_editor_content.text();
            let project_path = join(&path, &project_name)?;
            create_dir(&project_path)?;
            init_working_dir(Some(instruction), &project_path, context.selected_model)?;
            return Ok(Task::done(IcedMessage::Launch { path: project_path }));
        },
        IcedMessage::Init { path } => {
            let instruction = context.long_text_editor_content.text();
            init_working_dir(Some(instruction), &path, context.selected_model)?;
            return Ok(Task::done(IcedMessage::Launch { path }));
        },
        IcedMessage::Launch { .. } => unreachable!(),
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::EditShortText(a) => {
            context.short_text_editor_content.perform(a);
        },
        IcedMessage::SelectModel(m) => {
            context.selected_model = m;
        },
        IcedMessage::Error(_) => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut entries: Vec<Element<IcedMessage>> = context.entries.iter().enumerate().map(
        |(i, entry)| render_entry(i, entry, context)
    ).collect();

    // It makes rooms for popups when there're not enough entries.
    entries.push(text!("").width(context.window_size.width).height(context.window_size.height).into());

    let entries_stretched = Column::from_vec(entries)
        .padding(context.zoom * 8.0)
        .spacing(context.zoom * 8.0);

    let mut entries_scrollable = Scrollable::new(entries_stretched).id(context.entry_view_id.clone());

    if context.curr_popup.is_none() {
        entries_scrollable = entries_scrollable.on_scroll(|v| IcedMessage::EntryViewScrolled(v.absolute_offset()));
    }

    let entries_colored = Container::new(entries_scrollable).style(|_| set_bg(black()));
    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.cwd).size(context.zoom * 14.0)).padding(context.zoom * 8.0).into(),
        render_buttons(context),
        entries_colored.into(),
    ]);

    let mut full_view_stacked: Element<IcedMessage> = Container::new(full_view).into();

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
    } else if let Some(Popup::EntryError(_) | Popup::Preview { .. } | Popup::Help) = &context.curr_popup {
        let title = text!("{}", context.popup_title.clone().unwrap_or(String::new())).size(context.zoom * 14.0);

        if let Some((pre, trunc, post)) = &context.long_preview {
            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                popup(Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    Container::new(text!("{pre}").size(context.zoom * 14.0)).width(Length::Fill).style(|_| set_bg(gray(0.3))).into(),
                    text!("... ({} truncated) ...", prettify_bytes(*trunc as u64)).size(context.zoom * 14.0).into(),
                    Container::new(text!("{post}").size(context.zoom * 14.0)).width(Length::Fill).style(|_| set_bg(gray(0.3))).into(),
                ]).spacing(context.zoom * 20.0).width(Length::Fill)).width(Length::Fill).into(), context),
            ]).into();
        }

        else if let Some(image_buffer) = &context.image_buffer {
            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                popup(Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    ImageViewer::new(image_buffer.clone()).into(),
                ]).spacing(context.zoom * 20.0).width(Length::Fill)).width(Length::Fill).into(), context),
            ]).into();
        }

        else {
            let text_editor = TextEditor::new(&context.long_text_editor_content).size(context.zoom * 14.0).highlight(
                &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
                iced::highlighter::Theme::SolarizedDark,
            );

            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                popup(Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    text_editor.into(),
                ]).spacing(context.zoom * 20.0).width(Length::Fill)).width(Length::Fill).into(), context),
            ]).into();
        }
    } else if let Some(Popup::EntryError(e)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(text!("{e}").size(context.zoom * 14.0).into(), context).into(),
        ]).into();
    } else if let Some(Popup::AskDelete { is_dir, path }) = &context.curr_popup {
        let ask = if *is_dir {
            Column::from_vec(vec![
                text!("Delete directory `{path}`?").size(context.zoom * 14.0).into(),
                button("(Y)es", IcedMessage::DeleteDirectory(path.to_string()), green(), context.zoom).into(),
            ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill)
        } else {
            Column::from_vec(vec![
                text!("Delete file `{path}`?").size(context.zoom * 14.0).into(),
                button("(Y)es", IcedMessage::DeleteFile(path.to_string()), green(), context.zoom).into(),
            ]).spacing(context.zoom * 20.0).align_x(Horizontal::Center).width(Length::Fill)
        };

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(ask.into(), context).into(),
        ]).into();
    } else if let Some(Popup::Help) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(Scrollable::new(text!("{HELP_MESSAGE}").size(context.zoom * 14.0)).into(), context).into(),
        ]).into();
    }

    full_view_stacked.into()
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    if context.curr_popup.is_some() {
        return Container::new(text!("")).padding(context.zoom * 8.0).into();
    }

    let mut buttons: Vec<Element<IcedMessage>> = vec![button("(C)reate new", IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() }), green(), context.zoom).into()];

    if context.has_neukgu_index {
        buttons.push(button("(L)aunch", IcedMessage::Launch { path: context.cwd.clone() }, green(), context.zoom).into());
    } else {
        buttons.push(button("(I)nit here", IcedMessage::OpenPopup(Popup::Init { path: context.cwd.clone() }), green(), context.zoom).into());
    }

    buttons.push(button("(H)elp", IcedMessage::OpenPopup(Popup::Help), pink(), context.zoom).into());
    Row::from_vec(buttons).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into()
}

fn render_entry<'e, 'c, 'm>(index: usize, entry: &'e FileEntry, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let mut row = vec![];

    if let Some(i) = context.selected_entry && i == index {
        row.push(text!(">> ").size(context.zoom * 14.0).into());
    }

    row.push(button("Delete", IcedMessage::OpenPopup(Popup::AskDelete { is_dir: entry.is_dir, path: entry.path.to_string() }), red(), context.zoom).into());

    let char_count = entry.name.chars().map(
        |ch| match ch {
            '가'..='힣' => 10,
            _ => 7,
        }
    ).sum::<usize>() / 7;
    let is_dir = entry.is_dir;
    let is_hovered = if let Some(e) = &context.hovered_entry { e == &entry.name } else { false };
    let truncated_name = if char_count < 42 {
        format!(
            "{}{}{}",
            entry.name,
            if entry.is_dir { "/" } else { " " },
            " ".repeat(42 - char_count),
        )
    } else {
        format!(
            "{}...{}",
            entry.name.chars().take(39).collect::<String>(),
            if is_dir { "/" } else { " " },
        )
    };
    let size = match entry.size {
        Some(s) => {
            let s = prettify_bytes(s);
            format!("{s}{}", " ".repeat(14 - s.len()))
        },
        None => " ".repeat(14),
    };

    let mut truncated_name = text!("{truncated_name} {size}").size(context.zoom * 14.0);

    if is_hovered {
        truncated_name = truncated_name.color(black());
    } else if entry.error.is_none() {
        //
    } else {
        truncated_name = truncated_name.color(gray(0.8));
    };

    let name_bg_color = if is_hovered {
        gray(0.7)
    } else if entry.error.is_none() {
        gray(0.3)
    } else {
        gray(0.5)
    };

    let name_container = Container::new(truncated_name).padding(context.zoom * 8.0).style(
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
    let name_container: Element<IcedMessage> = if entry.error.is_none() && context.curr_popup.is_none() {
        MouseArea::new(name_container)
            .on_enter(IcedMessage::HoverOnEntry(Some(entry.name.to_string())))
            .on_exit(IcedMessage::HoverOnEntry(None))
            .on_press(if entry.is_dir {
                IcedMessage::ChDir(entry.path.to_string())
            } else {
                IcedMessage::OpenPopup(Popup::Preview { path: entry.path.to_string() })
            })
            .into()
    } else {
        name_container.into()
    };

    row.push(name_container.into());

    if let Some(e) = &entry.error {
        row.push(button("(!)", IcedMessage::OpenPopup(Popup::EntryError(e.to_string())), red(), context.zoom).into());
    }

    if entry.has_neukgu_index {
        row.push(disabled_button("  ", green(), context.zoom).into());
    }

    Row::from_vec(row).spacing(context.zoom * 12.0).align_y(Vertical::Center).into()
}

fn render_init_popup<'p, 'c>(path: &'p str, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let text_editor = TextEditor::new(&context.long_text_editor_content)
        .size(context.zoom * 14.0)
        .placeholder("What do you want neukgu to do?")
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    let model_selector = Row::from_vec(Model::all().into_iter().map(
        |m| Radio::new(m.short_name(), m, Some(context.selected_model), |m| IcedMessage::SelectModel(m)).into()
    ).collect());

    popup(
        Scrollable::new(
            Column::from_vec(vec![
                text_editor.into(),
                model_selector.into(),
                button("Init", IcedMessage::Init { path: path.to_string() }, green(), context.zoom).padding(context.zoom * 20.0).into(),
            ])
                .spacing(context.zoom * 20.0)
                .align_x(Horizontal::Center)
                .width(Length::Fill),
        )
            .width(Length::Fill)
            .into(),
        context,
    )
}

fn render_create_popup<'p, 'c>(path: &'p str, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let short_text_editor = TextEditor::new(&context.short_text_editor_content)
        .size(context.zoom * 14.0)
        .placeholder("Name of the project")
        .on_action(|action| IcedMessage::EditShortText(action));

    let long_text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .size(context.zoom * 14.0)
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    let model_selector = Row::from_vec(Model::all().into_iter().map(
        |m| Radio::new(m.short_name(), m, Some(context.selected_model), |m| IcedMessage::SelectModel(m)).into()
    ).collect());

    popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                long_text_editor.into(),
                model_selector.into(),
                button("Create", IcedMessage::Create { path: path.to_string() }, green(), context.zoom).padding(context.zoom * 20.0).into(),
            ])
                .spacing(context.zoom * 20.0)
                .align_x(Horizontal::Center)
                .width(Length::Fill),
        )
            .width(Length::Fill)
            .into(),
        context,
    )
}

fn load_entries(path: &str) -> Result<Vec<FileEntry>, Error> {
    let mut dirs = vec![FileEntry {
        name: String::from(".."),
        path: normalize_path(&join(path, "..")?)?,
        is_dir: true,
        has_neukgu_index: false,
        size: None,
        error: None,
    }];
    let mut files = vec![];

    for e in read_dir(path, true)? {
        let (has_neukgu_index, mut error) = match check_neukgu_index(&e) {
            Ok(h) => (h, None),
            Err(e) => (false, Some(format!("{e:?}"))),
        };

        if is_dir(&e) {
            dirs.push(FileEntry {
                name: basename(&e)?,
                path: e.to_string(),
                is_dir: true,
                has_neukgu_index,
                size: None,
                error,
            });
        }

        else {
            let size = match file_size(&e) {
                Ok(s) => Some(s),
                Err(e) => {
                    if error.is_none() {
                        error = Some(format!("{e:?}"));
                    }

                    None
                },
            };

            files.push(FileEntry {
                name: basename(&e)?,
                path: e.to_string(),
                is_dir: false,
                has_neukgu_index,
                size,
                error,
            });
        }
    }

    Ok(vec![dirs, files].concat())
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

fn popup<'e, 'c>(element: Element<'e, IcedMessage>, context: &'c IcedContext) -> Element<'e, IcedMessage> {
    let mut buttons: Vec<Element<IcedMessage>> = vec![];
    buttons.push(button("Close", IcedMessage::ClosePopup, red(), context.zoom).into());

    if context.copy_buffer.is_some() {
        buttons.push(button("Copy", IcedMessage::CopyToClipboard, blue(), context.zoom).into());
    }

    Container::new(
        Container::new(Column::from_vec(vec![
            Row::from_vec(buttons).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
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
        .padding(context.zoom * 8.0)
        .width(Length::Fill)
    )
    .style(|_| set_bg(Color::from_rgba(0.0, 0.0, 0.0, 0.5)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(context.zoom * 32.0)
    .into()
}

fn dump_hex(bytes: &[u8], offset: usize) -> String {
    bytes.chunks(16).enumerate().map(
        |(i, bytes)| {
            let mut pre = bytes[..bytes.len().min(8)].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
            let mut post = if bytes.len() < 8 {
                String::new()
            } else {
                bytes[8..].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
            };

            if pre.len() < 23 {
                pre = format!("{pre}{}", " ".repeat(23 - pre.len()));
            }

            if post.len() < 23 {
                post = format!("{post}{}", " ".repeat(23 - post.len()));
            }

            let mut pre_ascii = bytes[..bytes.len().min(8)].iter().map(
                |b| match b {
                    0 => '.',
                    ..32 => '_',
                    32..127 => *b as char,
                    _ => '_',
                }
            ).collect::<String>();
            let mut post_ascii = if bytes.len() < 8 {
                String::new()
            } else {
                bytes[8..].iter().map(
                    |b| match b {
                        0 => '.',
                        ..32 => '_',
                        32..127 => *b as char,
                        _ => '_',
                    }
                ).collect::<String>()
            };

            if pre_ascii.len() < 8 {
                pre_ascii = format!("{pre_ascii}{}", " ".repeat(8 - pre_ascii.len()));
            }

            if post_ascii.len() < 8 {
                post_ascii = format!("{post_ascii}{}", " ".repeat(8 - post_ascii.len()));
            }

            format!("{:08x} | {pre}    {post} | {pre_ascii}  {post_ascii} \n", i * 16 + offset)
        }
    ).collect::<Vec<_>>().concat()
}
