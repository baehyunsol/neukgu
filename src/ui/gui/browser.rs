use super::{
    black,
    blue,
    button,
    count_chars,
    disabled_button,
    gray,
    green,
    pink,
    red,
    set_bg,
    skyblue,
    take_chars,
    white,
};
use super::config::{SetProjectConfig, config_ui, set_project_config};
use super::popup::{PopupContext, PopupMessage, into_popup};
use super::worker::{
    Job,
    JobId,
    JobKind,
    JobResult,
    JobResultKind,
    RgMatch,
};
use crate::{
    Config,
    Error,
    init_working_dir,
    prettify_bytes,
    render_first_10_pages,
    validate_project_name,
};
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Button, Column, Id, MouseArea, Row, Scrollable, Space, Stack, TextInput, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{
    Handle as ImageHandle,
    Viewer as ImageViewer,
};
use iced::widget::operation::{AbsoluteOffset, focus, scroll_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    TextEditor,
};
use ragit_fs::{
    basename,
    create_dir,
    exists,
    extension,
    file_size,
    is_dir,
    join,
    parent,
    read_bytes,
    read_bytes_offset,
    read_dir,
    read_string,
    remove_dir_all,
    remove_file,
};
use std::sync::Arc;

const HELP_MESSAGE: &str = r#"
# File Browser & Viewer

## Neukgu working dir

You can create or initialize a neukgu working directory in file browser.

1. Creating a new project will create a directory and initialize a neukgu working directory.
2. `Init here` button will turn the current directory into a neukgu working directory.

## Key bindings

- Esc: close popup
- Alt+Up: `cd ..`
- (Ctrl)+Up/Down: prev/next file entry (Ctrl to move faster)
- Ctrl+Plus/Minus: zoom
- Ctrl+C: create working dir
- Ctrl+H: help message
- Ctrl+I: init working dir
- Ctrl+L: launch working dir
- Ctrl+T: new tab
- Ctrl+W: close tab
- Ctrl+Y: yes (confirm popup)
- Alt+Num: switch tab
- Enter: select file entry
- Delete: delete selected file entry
"#;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub home_dir: String,
    pub cwd: String,
    pub entries: Vec<FileEntry>,
    pub has_neukgu_index: bool,
    pub window_size: Size,
    pub entry_view_id: Id,
    pub short_text_editor_id: Id,
    pub long_text_editor_id: Id,
    pub entry_view_scrolled: AbsoluteOffset,

    // hovered_entry: mouse
    // selected_entry: arrow keys
    pub hovered_entry: Option<String>,
    pub selected_entry: Option<usize>,

    pub curr_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub image_buffer: Vec<(String, ImageHandle)>,
    pub syntax_highlight: Option<String>,
    pub long_preview: Option<(String, usize, String)>,
    pub popup_title: Option<String>,
    pub zoom: f32,
    pub new_project_config: Config,
    pub short_text_editor_content: String,
    pub long_text_editor_content: TextEditorContent,
}

impl IcedContext {
    pub fn new(home_dir: &str, cwd: &str, file: &Option<String>, window_size: Size) -> Result<IcedContext, Error> {
        let file = match file {
            Some(file) => Some(basename(file)?),
            None => None,
        };

        let mut context = IcedContext {
            home_dir: home_dir.to_string(),
            cwd: cwd.to_string(),
            entries: load_entries(cwd)?,
            has_neukgu_index: check_neukgu_index(cwd)?,
            window_size,
            entry_view_id: Id::unique(),
            short_text_editor_id: Id::unique(),
            long_text_editor_id: Id::unique(),
            entry_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            hovered_entry: None,
            selected_entry: None,
            curr_popup: None,
            copy_buffer: None,
            image_buffer: vec![],
            syntax_highlight: None,
            long_preview: None,
            popup_title: None,
            zoom: 1.0,
            new_project_config: Config::default(),
            short_text_editor_content: String::new(),
            long_text_editor_content: TextEditorContent::new(),
        };

        if let Some(file) = &file {
            context.open_popup(Popup::Preview { path: join(cwd, file)? })?;
        }

        Ok(context)
    }

    pub fn get_open_dir_and_file(&self) -> Result<(String, Option<String>), Error> {
        Ok((
            self.cwd.to_string(),
            if let Some(Popup::Preview { path }) = &self.curr_popup {
                Some(basename(path)?)
            } else {
                None
            },
        ))
    }

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
            Popup::Init { path } => {
                let instruction_at = join(&path, "neukgu-instruction.md")?;

                if exists(&instruction_at) {
                    self.set_text_editor_content(read_string(&instruction_at)?);
                }
            },
            Popup::EntryError(e) => {
                self.copy_buffer = Some(e.to_string());
                self.set_text_editor_content(e.to_string());
                self.syntax_highlight = None;
            },
            Popup::Preview { path } => {
                let mut is_binary = false;
                let file_size = file_size(&path)? as usize;
                let content: Option<String> = if file_size > 33554432 {
                    is_binary = true;
                    let pre = read_bytes_offset(&path, 0, 16384)?;
                    let mut post_offset = file_size - 16384;
                    post_offset -= post_offset % 32;
                    let post = read_bytes_offset(&path, post_offset as u64, file_size as u64)?;
                    Some(vec![
                        dump_hex(&pre, 0),
                        dump_hex(&post, post_offset),
                    ].concat())
                } else {
                    match String::from_utf8(read_bytes(&path)?) {
                        Ok(s) => {
                            self.syntax_highlight = extension(&path)?;
                            Some(s)
                        },
                        Err(e) => match image::load_from_memory(e.as_bytes()) {
                            Ok(img) => {
                                self.image_buffer = vec![(format!("{}x{}", img.width(), img.height()), ImageHandle::from_bytes(e.as_bytes().to_vec()))];
                                None
                            },
                            _ => match render_first_10_pages(e.as_bytes()) {
                                Ok(Some((pages, total_pages))) => {
                                    self.image_buffer = pages.into_iter().enumerate().map(|(i, buffer)| (format!("{}/{total_pages}", i + 1), ImageHandle::from_bytes(buffer))).collect();
                                    None
                                },
                                _ => {
                                    is_binary = true;
                                    Some(dump_hex(e.as_bytes(), 0))
                                },
                            },
                        },
                    }
                };

                let preview = match &content {
                    Some(content) if content.chars().count() > 32768 => {
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
                        None
                    },
                    Some(content) => Some(content.to_string()),
                    None => None,
                };

                self.popup_title = Some(path);

                if let Some(content) = content {
                    self.copy_buffer = Some(content);
                }

                if let Some(preview) = preview {
                    self.set_text_editor_content(preview);
                }
            },
            Popup::AskDelete { .. } => {},
            Popup::Find { .. } => {},
            Popup::FindResult { .. } => {},
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
        self.image_buffer = vec![];
        self.long_preview = None;
        self.popup_title = None;
        self.short_text_editor_content = String::new();
        self.long_text_editor_content = TextEditorContent::with_text("");
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.long_text_editor_content.perform(TextEditorAction::SelectAll);
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }
}

impl PopupContext for IcedContext {
    fn can_close_popup(&self) -> bool { true }
    fn has_prev_popup(&self) -> bool { false }
    fn has_something_to_copy(&self) -> bool { self.copy_buffer.is_some() }
    fn zoom(&self) -> f32 { self.zoom }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    EntryViewScrolled(AbsoluteOffset),
    HoverOnEntry(Option<String>),
    OpenPopup(Popup),
    ClosePopup,
    CopyPopupContent,
    ChDir(String),
    DeleteFile(String),
    DeleteDirectory(String),
    Create { path: String },
    Init { path: String },
    Launch { path: String },
    NewBrowser { dir: String, file: Option<String> },
    Find,
    EditShortText(String),
    EditLongText(TextEditorAction),
    FocusShortTextEdit,
    FocusLongTextEdit,
    SetProjectConfig(SetProjectConfig),
    Error(String),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Focus,

    // Kill: The caller wants to kill this tab.
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    Dead,
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { unreachable!() }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
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
    Find {
        error: Option<String>,

        // It'll be set when the background worker starts working.
        job: Option<JobId>,
    },
    FindResult {
        regex: String,
        matches: Vec<RgMatch>,
        truncate: Option<usize>,
        match_count: usize,
    },
    Help,
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick { frame, force_update } => {
            if frame % 4 == 0 || force_update {
                if context.curr_popup.is_none() {
                    context.entries = load_entries(&context.cwd)?;
                    context.has_neukgu_index = check_neukgu_index(&context.cwd)?;
                }
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
            (Key::Character("c"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() })));
                }
            },
            (Key::Character("f"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Find { error: None, job: None })));
                }
            },
            (Key::Character("h"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Help)));
                }
            },
            (Key::Character("i"), true, false, false) => {
                if context.curr_popup.is_none() && !context.has_neukgu_index {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Init { path: context.cwd.clone() })));
                }
            },
            (Key::Character("l"), true, false, false) => {
                if context.curr_popup.is_none() && context.has_neukgu_index {
                    return Ok(Task::done(IcedMessage::Launch { path: context.cwd.clone() }));
                }
            },
            (Key::Character("y"), true, false, false) => {
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
            let mut tasks: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled),
            ];

            match &popup {
                Popup::Create { .. } | Popup::Find { .. } => {
                    tasks.push(focus(context.short_text_editor_id.clone()));
                },
                Popup::Init { .. } => {
                    tasks.push(focus(context.long_text_editor_id.clone()));
                },
                _ => {},
            }

            context.open_popup(popup)?;
            return Ok(Task::batch(tasks));
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::CopyPopupContent => {
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
            let project_name = context.short_text_editor_content.to_string();
            validate_project_name(&project_name)?;
            let instruction = context.long_text_editor_content.text();
            let project_path = join(&path, &project_name)?;
            create_dir(&project_path)?;
            init_working_dir(Some(instruction), &project_path, context.new_project_config.clone(), false)?;
            return Ok(Task::done(IcedMessage::Launch { path: project_path }));
        },
        IcedMessage::Init { path } => {
            let instruction = context.long_text_editor_content.text();
            init_working_dir(Some(instruction), &path, context.new_project_config.clone(), false)?;
            return Ok(Task::done(IcedMessage::Launch { path }));
        },
        IcedMessage::Launch { .. } => unreachable!(),
        IcedMessage::NewBrowser { .. } => unreachable!(),
        IcedMessage::Find => {
            let job_id = JobId::new();

            if let Some(Popup::Find { job, .. }) = &mut context.curr_popup {
                *job = Some(job_id);
            }

            return Ok(Task::done(IcedMessage::BackgroundJob(Job {
                id: job_id,
                kind: JobKind::Rg {
                    path: context.cwd.to_string(),
                    regex: context.short_text_editor_content.to_string(),
                },
            })));
        },
        IcedMessage::EditShortText(s) => {
            context.short_text_editor_content = s;
        },
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::FocusShortTextEdit => {
            return Ok(focus(context.short_text_editor_id.clone()));
        },
        IcedMessage::FocusLongTextEdit => {
            return Ok(focus(context.long_text_editor_id.clone()));
        },
        IcedMessage::SetProjectConfig(c) => {
            set_project_config(&mut context.new_project_config, c);
        },
        IcedMessage::Focus => {
            return Ok(scroll_to(context.entry_view_id.clone(), context.entry_view_scrolled));
        },
        IcedMessage::Error(_) => unreachable!(),
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(job_result) => match &job_result.kind {
            JobResultKind::RgTimeout => {
                match &mut context.curr_popup {
                    Some(Popup::Find { error, job }) if *job == job_result.id => {
                        *job = None;
                        *error = Some(String::from("ripgrep timeout"));
                    },
                    _ => {},
                }
            },
            JobResultKind::Rg { regex, matches, count } => match &context.curr_popup {
                Some(Popup::Find { job, .. }) if *job == job_result.id => {
                    let (matches, truncate) = if matches.len() < 512 {
                        (matches.to_vec(), None)
                    } else {
                        (matches[..512].to_vec(), Some(matches.len() - 512))
                    };

                    context.open_popup(Popup::FindResult { regex: regex.to_string(), matches, truncate, match_count: *count })?;
                },
                _ => {},
            },
            _ => {},
        },
        IcedMessage::Kill => {
            return Ok(Task::done(IcedMessage::Dead));
        },
        IcedMessage::Dead => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut entries: Vec<Element<IcedMessage>> = context.entries.iter().enumerate().map(
        |(i, entry)| render_entry(i, entry, context)
    ).collect();

    // It makes rooms for popups when there're not enough entries.
    entries.push(text!("").width(context.window_size.width).height(context.window_size.height * 0.5).into());

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
    } else if let Some(Popup::Find { error, job }) = &context.curr_popup {
        let text_editor = TextInput::new("regex", &context.short_text_editor_content)
            .size(context.zoom * 14.0)
            .id(context.short_text_editor_id.clone())
            .on_input(|input| IcedMessage::EditShortText(input))
            .on_submit(IcedMessage::Find);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(
                Column::from_vec(vec![
                    text_editor.into(),
                    if job.is_some() {
                        text!("Finding...").size(context.zoom * 14.0).into()
                    } else {
                        Space::new().into()
                    },
                    if let Some(error) = error {
                        text!("{error}").size(context.zoom * 14.0).color(red()).into()
                    } else {
                        Space::new().into()
                    },
                    if job.is_some() {
                        disabled_button("Find", green(), context.zoom).padding(context.zoom * 20.0).into()
                    } else {
                        button("Find", IcedMessage::Find, green(), context.zoom).padding(context.zoom * 20.0).into()
                    },
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .into(),
                context,
            ).into(),
        ]).into();
    } else if let Some(Popup::FindResult { regex, matches, truncate, match_count }) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(render_find_result(regex, matches, *truncate, *match_count, context), context).into(),
        ]).into();
    } else if let Some(Popup::EntryError(_) | Popup::Preview { .. } | Popup::Help) = &context.curr_popup {
        let title = text!("{}", context.popup_title.clone().unwrap_or(String::new())).size(context.zoom * 18.0);

        if let Some((pre, trunc, post)) = &context.long_preview {
            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                into_popup(Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    Container::new(text!("{pre}").size(context.zoom * 14.0)).width(Length::Fill).style(|_| set_bg(gray(0.3))).into(),
                    text!("... ({} truncated) ...", prettify_bytes(*trunc as u64)).size(context.zoom * 14.0).into(),
                    Container::new(text!("{post}").size(context.zoom * 14.0)).width(Length::Fill).style(|_| set_bg(gray(0.3))).into(),
                ]).spacing(context.zoom * 20.0).width(Length::Fill)).width(Length::Fill).into(), context),
            ]).into();
        }

        else if !context.image_buffer.is_empty() {
            let mut column: Vec<Element<IcedMessage>> = vec![title.into()];

            for (caption, image) in context.image_buffer.iter() {
                column.push(text!("{caption}").size(context.zoom * 14.0).into());
                column.push(ImageViewer::new(image.clone()).into());
            }

            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                into_popup(Scrollable::new(
                    Column::from_vec(column)
                        .spacing(context.zoom * 20.0)
                        .width(Length::Fill)
                ).width(Length::Fill).into(), context),
            ]).into();
        }

        else {
            let text_editor = TextEditor::new(&context.long_text_editor_content).size(context.zoom * 14.0).highlight(
                &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
                iced::highlighter::Theme::SolarizedDark,
            );

            full_view_stacked = Stack::from_vec(vec![
                full_view_stacked,
                into_popup(Scrollable::new(Column::from_vec(vec![
                    title.into(),
                    text_editor.into(),
                ]).spacing(context.zoom * 20.0).width(Length::Fill)).width(Length::Fill).into(), context),
            ]).into();
        }
    } else if let Some(Popup::EntryError(e)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(text!("{e}").size(context.zoom * 14.0).into(), context).into(),
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
            into_popup(ask.into(), context).into(),
        ]).into();
    } else if let Some(Popup::Help) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(text!("{HELP_MESSAGE}").size(context.zoom * 14.0)).into(), context).into(),
        ]).into();
    }

    full_view_stacked.into()
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let mut buttons_row1: Vec<Button<IcedMessage>> = if context.has_neukgu_index {
        vec![
            button("(C)reate new", IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() }), green(), context.zoom).into(),
            button("(L)aunch", IcedMessage::Launch { path: context.cwd.clone() }, green(), context.zoom).into(),
        ]
    } else {
        vec![
            button("(C)reate new", IcedMessage::OpenPopup(Popup::Create { path: context.cwd.clone() }), green(), context.zoom).into(),
            button("(I)nit here", IcedMessage::OpenPopup(Popup::Init { path: context.cwd.clone() }), green(), context.zoom).into(),
        ]
    };
    let mut buttons_row2: Vec<Button<IcedMessage>> = vec![];

    buttons_row1.push(button("(H)elp", IcedMessage::OpenPopup(Popup::Help), pink(), context.zoom).into());

    if let Ok(parent) = parent(&context.cwd) && !parent.is_empty() {
        buttons_row2.push(button("Up", IcedMessage::ChDir(parent), blue(), context.zoom).into());
    }

    buttons_row2.push(button("Home", IcedMessage::ChDir(context.home_dir.to_string()), blue(), context.zoom).into());
    buttons_row2.push(button("(F)ind", IcedMessage::OpenPopup(Popup::Find { error: None, job: None }), blue(), context.zoom).into());

    let buttons_row1 = if context.curr_popup.is_some() {
        buttons_row1.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons_row1.into_iter().map(|button| button.into()).collect()
    };
    let buttons_row2 = if context.curr_popup.is_some() {
        buttons_row2.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
    } else {
        buttons_row2.into_iter().map(|button| button.into()).collect()
    };

    Column::from_vec(vec![
        Row::from_vec(buttons_row1).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
        Row::from_vec(buttons_row2).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into(),
    ]).into()
}

fn render_entry<'e, 'c, 'm>(index: usize, entry: &'e FileEntry, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let mut row = vec![];

    if let Some(i) = context.selected_entry && i == index {
        row.push(text!(">> ").size(context.zoom * 14.0).into());
    }

    if context.curr_popup.is_some() {
        row.push(disabled_button("Delete", red(), context.zoom).into());
    } else {
        row.push(button("Delete", IcedMessage::OpenPopup(Popup::AskDelete { is_dir: entry.is_dir, path: entry.path.to_string() }), red(), context.zoom).into());
    }

    let char_count = count_chars(&entry.name);
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
            take_chars(&entry.name, 39),
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
        if context.curr_popup.is_some() {
            row.push(disabled_button("(!)", red(), context.zoom).into());
        }

        else {
            row.push(button("(!)", IcedMessage::OpenPopup(Popup::EntryError(e.to_string())), red(), context.zoom).into());
        }
    }

    if entry.has_neukgu_index {
        row.push(disabled_button("  ", green(), context.zoom).into());
    }

    Row::from_vec(row).spacing(context.zoom * 12.0).align_y(Vertical::Center).into()
}

fn render_init_popup<'p, 'c>(path: &'p str, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .size(context.zoom * 14.0)
        .id(context.long_text_editor_id.clone())
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                text_editor.into(),
                config_ui(&context.new_project_config, context.zoom).map(|m| IcedMessage::SetProjectConfig(m)).into(),
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
    let short_text_editor = TextInput::new("Name of the project", &context.short_text_editor_content)
        .size(context.zoom * 14.0)
        .id(context.short_text_editor_id.clone())
        .on_input(|input| IcedMessage::EditShortText(input))
        .on_submit(IcedMessage::FocusLongTextEdit);

    let long_text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .size(context.zoom * 14.0)
        .id(context.long_text_editor_id.clone())
        .min_height(400)
        .on_action(|action| IcedMessage::EditLongText(action));

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                long_text_editor.into(),
                config_ui(&context.new_project_config, context.zoom).map(|m| IcedMessage::SetProjectConfig(m)).into(),
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
    let mut dirs = vec![];
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

fn render_find_result<'f, 'm, 'c>(
    regex: &'f str,
    matches: &'m [RgMatch],
    truncate: Option<usize>,
    match_count: usize,
    context: &'c IcedContext,
) -> Element<'c, IcedMessage> {
    let mut lines = vec![];
    let mut curr_path = String::new();
    let mut curr_line_no = 0;

    for m in matches.iter() {
        if curr_path != m.path {
            lines.push(
                Row::from_vec(vec![
                    text!("{}", m.path).size(context.zoom * 14.0).into(),
                    button("Open", IcedMessage::NewBrowser {
                        dir: join(&context.cwd, &parent(&m.path).unwrap()).unwrap(),
                        file: Some(basename(&m.path).unwrap()),
                    }, skyblue(), context.zoom).into(),
                ])
                    .padding(context.zoom * 8.0)
                    .spacing(context.zoom * 8.0)
                    .align_y(Vertical::Center)
                    .into()
            );
            curr_path = m.path.to_string();
        }

        else if m.line_number != curr_line_no + 1 {
            lines.push(Space::new().height(context.zoom * 14.0).into());
        }

        curr_line_no = m.line_number;
        let mut highlights = vec![
            text!("{:>6} | ", m.line_number).size(context.zoom * 14.0).into(),
        ];
        let mut curr_index = 0;
        let line = m.line.chars().take(256).collect::<String>();
        let line = line.replace("\r", " ").replace("\n", " ");
        let submatches: Vec<(usize, usize)> = m.submatches.iter().filter_map(
            |(start, end)| {
                let start = (*start).min(line.len());
                let end = (*end).min(line.len());

                if end >= line.len() {
                    None
                } else {
                    Some((start, end))
                }
            }
        ).collect();

        for (start, end) in submatches.iter() {
            if curr_index < *start {
                highlights.push(text!("{}", line.get(curr_index..*start).unwrap()).size(context.zoom * 14.0).into());
            }

            highlights.push(
                Container::new(
                    text!("{}", line.get(*start..*end).unwrap())
                        .color(black())
                        .size(context.zoom * 14.0)
                ).style(|_| set_bg(white())).into(),
            );
            curr_index = *end;
        }

        if curr_index < line.len() {
            highlights.push(text!("{}", line.get(curr_index..).unwrap().trim_end()).size(context.zoom * 14.0).into());
        }

        lines.push(Row::from_vec(highlights).into());
    }

    Column::from_vec(vec![
        text!("find {regex:?}").size(context.zoom * 14.0).into(),
        text!(
            "{} result{}{}",
            match_count,
            if match_count == 1 { "" } else { "s" },
            if let Some(truncate) = truncate { format!(" (truncated {truncate} lines)") } else { String::new() },
        ).size(context.zoom * 14.0).into(),
        Scrollable::new(
            Column::from_vec(lines).width(context.window_size.width)
        ).into(),
    ]).padding(context.zoom * 8.0).spacing(context.zoom * 8.0).into()
}
