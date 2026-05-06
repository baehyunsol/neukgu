use super::{
    PopupContext,
    PopupMessage,
    black,
    button,
    circle,
    disabled_button,
    gray,
    green,
    into_popup,
    red,
    skyblue,
    white,
    yellow,
};
use super::tab::{TabId, TabPreview};
use super::tabs::Tab;
use chrono::Local;
use crate::{
    Error,
    Model,
    Project,
    clean_global_index_dir,
    get_global_index_dir,
    get_neukgu_id,
    init_working_dir,
    load_all_indexes,
    prettify_time,
    remove_global_index,
    validate_project_name,
};
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, MouseArea, Radio, Row, Scrollable, Space, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    TextEditor,
};
use ragit_fs::{
    basename,
    create_dir,
    join,
    join3,
    read_string,
    remove_dir_all,
};
use std::sync::Arc;

pub struct IcedContext {
    pub home_dir: String,
    pub global_index_dir: String,
    pub window_size: Size,
    pub recent_projects: Vec<Project>,
    pub current_tabs: Vec<TabPreview>,
    pub zoom: f32,
    pub now: String,
    pub hovered_tab: Option<TabId>,
    pub curr_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub syntax_highlight: Option<String>,

    pub short_text_editor_content: TextEditorContent,
    pub long_text_editor_content: TextEditorContent,
    pub selected_model: Model,
}

impl IcedContext {
    pub fn new(home_dir: &str) -> IcedContext {
        let global_index_dir = get_global_index_dir().unwrap();
        clean_global_index_dir(&global_index_dir).unwrap();

        IcedContext {
            home_dir: home_dir.to_string(),
            global_index_dir,
            window_size: Size::new(0.0, 0.0),
            recent_projects: vec![],
            current_tabs: vec![],
            zoom: 1.0,
            now: Local::now().to_rfc2822(),
            hovered_tab: None,
            curr_popup: None,
            copy_buffer: None,
            syntax_highlight: None,
            short_text_editor_content: TextEditorContent::new(),
            long_text_editor_content: TextEditorContent::new(),
            selected_model: Model::default(),
        }
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::NewProject => {},
            Popup::Instruction { working_dir } => {
                let instruction = read_string(&join(&working_dir, "neukgu-instruction.md")?)?;
                self.copy_buffer = Some(instruction.to_string());
                self.set_long_text_editor_content(instruction);
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::AskDelete { .. } => {},
            Popup::Error(_) => {},
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
        self.curr_popup = None;
        self.copy_buffer = None;
        self.short_text_editor_content = TextEditorContent::with_text("");
        self.long_text_editor_content = TextEditorContent::with_text("");
        self.syntax_highlight = None;
    }

    pub fn set_long_text_editor_content(&mut self, c: String) {
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
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    HoverOnTab(Option<TabId>),
    NewTab(Tab),
    OpenTab { id: TabId, index: usize },
    NewProject,
    DeleteProject {
        project_name: String,
        working_dir: String,
    },
    OpenPopup(Popup),
    ClosePopup,
    CopyPopupContent,
    EditLongText(TextEditorAction),
    EditShortText(TextEditorAction),
    SelectModel(Model),
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { unreachable!() }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
}

#[derive(Clone, Debug)]
pub enum Popup {
    NewProject,
    Instruction {
        working_dir: String,
    },
    AskDelete {
        project_name: String,
        working_dir: String,
    },
    Error(String),
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::OpenPopup(Popup::Error(format!("{e:?}")))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick => {
            context.now = Local::now().to_rfc2822();
            context.recent_projects = load_all_indexes(&context.global_index_dir);

            // let's just assume that there's no overflow!
            context.recent_projects.sort_by_key(|p| -p.updated_at);
        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Escape), false, false, false) => {
                return Ok(Task::done(IcedMessage::ClosePopup));
            },
            (Key::Character("p"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::NewProject)));
                }
            },
            (Key::Character("t"), false, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None })));
                }
            },
            (Key::Character("y"), false, false, false) => {
                if let Some(Popup::AskDelete { project_name, working_dir }) = &context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteProject { project_name: project_name.to_string(), working_dir: working_dir.to_string() }));
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
        IcedMessage::HoverOnTab(id) => {
            context.hovered_tab = id;
        },
        IcedMessage::NewTab(_) => unreachable!(),
        IcedMessage::OpenTab { .. } => unreachable!(),
        IcedMessage::NewProject => {
            let project_name = context.short_text_editor_content.text();
            validate_project_name(&project_name)?;
            let instruction = context.long_text_editor_content.text();
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;
            create_dir(&project_path)?;
            init_working_dir(Some(instruction), &project_path, context.selected_model, true)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::NewTab(Tab::WorkingDir(project_path))));
        },
        IcedMessage::DeleteProject { project_name, working_dir } => {
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;
            let neukgu_id = get_neukgu_id(&working_dir)?;
            remove_dir_all(&project_path)?;
            remove_global_index(&context.global_index_dir, neukgu_id)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::OpenPopup(popup) => {
            // TODO: we need an error handler for index tab
            context.open_popup(popup)?;
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
        },
        IcedMessage::CopyPopupContent => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::EditShortText(a) => {
            context.short_text_editor_content.perform(a);
        },
        IcedMessage::SelectModel(m) => {
            context.selected_model = m;
        },
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let c = Column::from_vec(vec![
        text!("{}", context.now).size(context.zoom * 14.0).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("Recent projects").size(context.zoom * 14.0).into(),
                if context.curr_popup.is_some() {
                    disabled_button("New (p)roject", green(), context.zoom).into()
                } else {
                    button("New (p)roject", IcedMessage::OpenPopup(Popup::NewProject), green(), context.zoom).into()
                },
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
                if context.curr_popup.is_some() {
                    disabled_button("New (t)ab", skyblue(), context.zoom).into()
                } else {
                    button("New (t)ab", IcedMessage::NewTab(Tab::Browser { dir: context.home_dir.to_string(), file: None }), skyblue(), context.zoom).into()
                },
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

    let mut full_view_stacked: Element<IcedMessage> = Scrollable::new(c).into();

    if let Some(Popup::NewProject) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_new_project_popup(context),
        ]).into();
    }

    else if let Some(Popup::Error(e)) = &context.curr_popup {
        let error = Column::from_vec(vec![
            text!("ERROR").color(red()).size(context.zoom * 21.0).into(),
            text!("{e}").size(context.zoom * 14.0).into(),
        ])
            .padding(context.zoom * 20.0)
            .spacing(context.zoom * 20.0)
            .align_x(Horizontal::Center)
            .width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(error.into(), context),
        ]).into();
    }

    else if let Some(Popup::AskDelete { project_name, working_dir }) = &context.curr_popup {
        let ask = Column::from_vec(vec![
            text!("Delete project `{project_name}`?").size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::DeleteProject { project_name: project_name.to_string(), working_dir: working_dir.to_string() }, green(), context.zoom).into(),
        ])
            .spacing(context.zoom * 20.0)
            .align_x(Horizontal::Center)
            .width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(ask.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Instruction { .. }) = context.curr_popup {
        let text_editor = TextEditor::new(&context.long_text_editor_content).size(context.zoom * 14.0).highlight(
            &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
            iced::highlighter::Theme::SolarizedDark,
        );

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(text_editor).width(Length::Fill).into(), context),
        ]).into();
    }

    full_view_stacked
}

fn render_projects<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let now = Local::now().timestamp_millis();
    let column: Vec<Element<IcedMessage>> = context.recent_projects.iter().map(
        |project| {
            let path = if project.is_in_global_index_dir {
                basename(&project.working_dir).unwrap()
            } else {
                project.working_dir.to_string()
            };

            let elapsed = match now - project.updated_at {
                ..0 => String::from("past"),
                ..10_000 => String::from("now"),
                d => format!("{} ago", prettify_time(d as u64)),
            };
            let elapsed = if project.error.is_some() {
                String::new()
            } else {
                format!("({elapsed})")
            };

            Container::new(
                Column::from_vec(vec![
                    text!("{path} {elapsed}").size(context.zoom * 14.0).width(context.window_size.width).into(),
                    if let Some(error) = &project.error {
                        text!("{error}").size(context.zoom * 14.0).color(red()).into()
                    }
                    else if context.curr_popup.is_some() {
                        Row::from_vec(vec![
                            disabled_button("Launch", green(), context.zoom).into(),
                            disabled_button("Browse", skyblue(), context.zoom).into(),
                            disabled_button("Instruction", yellow(), context.zoom).into(),
                            if project.is_in_global_index_dir {
                                disabled_button("Delete", red(), context.zoom).into()
                            } else {
                                Space::new().into()
                            },
                        ]).spacing(context.zoom * 4.0).into()
                    } else {
                        Row::from_vec(vec![
                            button("Launch", IcedMessage::NewTab(Tab::WorkingDir(project.working_dir.to_string())), green(), context.zoom).into(),
                            button("Browse", IcedMessage::NewTab(Tab::Browser { dir: project.working_dir.to_string(), file: None }), skyblue(), context.zoom).into(),
                            button("Instruction", IcedMessage::OpenPopup(Popup::Instruction { working_dir: project.working_dir.to_string() }), yellow(), context.zoom).into(),
                            if project.is_in_global_index_dir {
                                button("Delete", IcedMessage::OpenPopup(Popup::AskDelete {
                                    project_name: path.to_string(),
                                    working_dir: project.working_dir.to_string(),
                                }), red(), context.zoom).into()
                            } else {
                                Space::new().into()
                            },
                        ]).spacing(context.zoom * 4.0).into()
                    },
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
    fn draw_bg(e: Row<IcedMessage>, is_hovered: bool, window_width: f32, zoom: f32) -> Container<IcedMessage> {
        Container::new(e).style(move |_| Style {
            background: Some(if is_hovered { Background::Color(gray(0.45)) } else { Background::Color(gray(0.15)) }),
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(zoom * 8.0),
            },
            ..Style::default()
        })
            .width(window_width)
            .padding(zoom * 8.0)
    }

    let mut column: Vec<Element<IcedMessage>> = vec![draw_bg(
        Row::from_vec(vec![
            text!("1. ").size(context.zoom * 14.0).into(),
            circle(context.zoom * 6.0, white()),
            text!("Index").size(context.zoom * 14.0).into(),
        ])
            .spacing(context.zoom * 4.0)
            .align_y(Vertical::Center),
        false,
        context.window_size.width,
        context.zoom,
    ).into()];

    column.extend(context.current_tabs.iter().enumerate().map(
        |(i, tab)| {
            let mut texts: Vec<Element<IcedMessage>> = vec![text!("{}", tab.title).size(context.zoom * 14.0).into()];

            if let Some(status) = &tab.status {
                texts.push(text!("{status}").size(context.zoom * 14.0).into());
            }

            if let Some(error) = &tab.error {
                texts.push(text!("{error}").color(red()).size(context.zoom * 14.0).into());
            }

            let tab_view = draw_bg(
                Row::from_vec(vec![
                    text!("{}. ", i + 2).size(context.zoom * 14.0).into(),
                    circle(context.zoom * 6.0, tab.flag),
                    if texts.len() == 1 { texts.remove(0).into() } else { Column::from_vec(texts).into() },
                ])
                    .spacing(context.zoom * 4.0)
                    .align_y(Vertical::Center),
                Some(tab.id) == context.hovered_tab && context.curr_popup.is_none(),
                context.window_size.width,
                context.zoom,
            );

            if context.curr_popup.is_some() {
                tab_view.into()
            } else {
                MouseArea::new(tab_view)
                    .on_enter(IcedMessage::HoverOnTab(Some(tab.id)))
                    .on_exit(IcedMessage::HoverOnTab(None))
                    .on_press(IcedMessage::OpenTab { id: tab.id, index: tab.index })
                    .into()
            }
        }
    ));

    Column::from_vec(column).spacing(context.zoom * 8.0).into()
}

// TODO: It's copy-paste of `ui::gui::browser::render_create_popup`. I have to create a generic function.
fn render_new_project_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
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

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                long_text_editor.into(),
                model_selector.into(),
                button("Create", IcedMessage::NewProject, green(), context.zoom).padding(context.zoom * 20.0).into(),
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
