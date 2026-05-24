use super::{
    black,
    blue,
    brown,
    button,
    circle,
    disabled_button,
    gray,
    green,
    pink,
    red,
    set_round_bg,
    skyblue,
    white,
    yellow,
};
use super::config::{
    SetChatConfig,
    SetProjectConfig,
    chat_config_ui1,
    chat_config_ui2,
    config_ui,
    set_chat_config,
    set_project_config,
};
use super::model_store::{self, IcedContext as ModelStoreContext, IcedMessage as ModelStoreMessage};
use super::popup::{PopupContext, PopupMessage, into_popup};
use super::scratch_pad::Content as ScratchPadContent;
use super::tab::{TabId, TabPreview};
use super::tabs::Tab;
use super::worker::JobResult;
use chrono::Local;
use crate::{
    Chat,
    ChatId,
    Config,
    Error,
    NeukguId,
    Project,
    clean_global_index_dir,
    delete_chat,
    get_global_chat_config,
    get_global_config,
    get_global_index_dir,
    get_neukgu_id,
    init_chat,
    init_global_index_dir,
    init_working_dir,
    load_all_chats,
    load_all_indexes,
    prettify_timestamp,
    remove_global_index,
    save_global_chat_config,
    save_global_config,
    validate_project_name,
};
use crate::chat::Config as ChatConfig;
use iced::{Background, Color, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Button, Column, Id, MouseArea, Row, Scrollable, Space, Stack, TextInput, text};
use iced::widget::container::{Container, Style};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, focus, scroll_to, snap_to};
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
use std::collections::HashMap;
use std::sync::Arc;

const HELP_MESSAGE: &str = r#"
# Neukgu

## Key bindings

- Esc: close popup
- Ctrl+Up/Down: scroll to top/bottom
- Ctrl+Plus/Minus: zoom
- Ctrl+C: new chat
- Ctrl+H: help message
- Ctrl+M: open model store
- Ctrl+P: new project
- Ctrl+T: new tab
- Ctrl+W: close tab
- Ctrl+Y: yes (confirm popup)
- Alt+Num: switch tab

## Key bindings (scratch pad)

- Ctrl+Shift+Esc: close scratch pad
- Ctrl+Shift+Up/Down: scroll scratch pad to top/bottom
- Ctrl+Shift+E: expand/collapse scratch pad
- Ctrl+Shift+Left/Right: move scratch pad
- Ctrl+Shift+Plus/Minus: zoom
"#;

pub struct IcedContext {
    pub home_dir: String,
    pub global_index_dir: String,
    pub window_size: Size,
    pub recent_projects: Vec<Project>,
    pub recent_chats: Vec<Chat>,
    pub current_tabs: Vec<TabPreview>,
    pub working_dir_tab_indexes: HashMap<NeukguId, usize>,
    pub main_view_id: Id,
    pub short_text_editor_id: Id,
    pub long_text_editor_id: Id,
    pub popup_scroll_id: Id,
    pub main_view_scrolled: AbsoluteOffset,
    pub zoom: f32,
    pub project_section_expanded: bool,
    pub chat_section_expanded: bool,
    pub tab_section_expanded: bool,
    pub now: String,
    pub battery: Option<(battery::State, f32)>,
    pub hovered_tab: Option<TabId>,
    pub curr_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub syntax_highlight: Option<String>,

    pub new_project_config: Config,
    pub new_chat_config: ChatConfig,
    pub model_store_context: ModelStoreContext,
    pub short_text_editor_content: String,
    pub long_text_editor_content: TextEditorContent,
}

impl IcedContext {
    pub fn new(home_dir: &str) -> IcedContext {
        let global_index_dir = get_global_index_dir().unwrap();
        init_global_index_dir(&global_index_dir).unwrap();
        clean_global_index_dir(&global_index_dir).unwrap();

        IcedContext {
            home_dir: home_dir.to_string(),
            global_index_dir: global_index_dir.to_string(),
            window_size: Size::new(0.0, 0.0),
            recent_projects: vec![],
            recent_chats: vec![],
            current_tabs: vec![],
            working_dir_tab_indexes: HashMap::new(),
            main_view_id: Id::unique(),
            short_text_editor_id: Id::unique(),
            long_text_editor_id: Id::unique(),
            popup_scroll_id: Id::unique(),
            main_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
            zoom: 1.0,
            project_section_expanded: false,
            chat_section_expanded: false,
            tab_section_expanded: false,
            now: Local::now().to_rfc2822(),
            battery: None,
            hovered_tab: None,
            curr_popup: None,
            copy_buffer: None,
            syntax_highlight: None,
            new_project_config: get_global_config(&global_index_dir).unwrap(),
            new_chat_config: get_global_chat_config(&global_index_dir).unwrap(),
            model_store_context: ModelStoreContext::new(global_index_dir.to_string()),
            short_text_editor_content: String::new(),
            long_text_editor_content: TextEditorContent::new(),
        }
    }

    pub fn update_battery_state(&mut self) {
        if let Ok(manager) = battery::Manager::new() {
            if let Ok(mut iterator) = manager.batteries() {
                if let Some(Ok(battery)) = iterator.next() {
                    let state = battery.state();
                    let charged = format!("{:?}", battery.state_of_charge()).parse::<f32>().unwrap();
                    self.battery = Some((state, charged));
                }
            }
        }
    }

    pub fn open_popup(&mut self, popup: Popup) -> Result<Task<IcedMessage>, Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::NewProject => {
                self.new_project_config = get_global_config(&self.global_index_dir)?;
            },
            Popup::ProjectConfig => {
                self.new_project_config = get_global_config(&self.global_index_dir)?;
            },
            Popup::NewChat => {
                self.new_chat_config = get_global_chat_config(&self.global_index_dir)?;
            },
            Popup::ChatConfig => {
                self.new_chat_config = get_global_chat_config(&self.global_index_dir)?;
            },
            Popup::Instruction { working_dir } => {
                let instruction = read_string(&join(&working_dir, "neukgu-instruction.md")?)?;
                self.copy_buffer = Some(instruction.to_string());
                self.set_long_text_editor_content(instruction);
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::AskDeleteProject { .. } => {},
            Popup::AskDeleteChat { .. } => {},
            Popup::ModelStore => {
                self.model_store_context.refresh()?;
                return Ok(model_store::update(&mut self.model_store_context, ModelStoreMessage::Focus)?.map(IcedMessage::UpdateModelStore));
            },
            Popup::Help => {
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
                self.set_long_text_editor_content(HELP_MESSAGE.to_string());
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::Error(_) => {},
        }

        Ok(Task::none())
    }

    pub fn close_popup(&mut self) {
        self.curr_popup = None;
        self.copy_buffer = None;
        self.short_text_editor_content = String::new();
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
    fn can_close_popup(&self) -> bool { self.curr_popup.is_some() }
    fn has_prev_popup(&self) -> bool { false }
    fn has_something_to_copy(&self) -> bool { self.copy_buffer.is_some() }

    fn can_open_scratch_pad(&self) -> bool {
        if let Some(c) = &self.copy_buffer && c.len() < 32768 {
            true
        } else {
            false
        }
    }

    fn zoom(&self) -> f32 { self.zoom }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    HoverOnTab(Option<TabId>),
    NewTab { tab: Tab, force_new_tab: bool },
    OpenTab { id: TabId, index: usize },
    ExpandProjectSection,
    ExpandChatSection,
    ExpandTabSection,
    NewProject,
    SaveGlobalProjectConfig,
    NewChat,
    SaveGlobalChatConfig,
    DeleteProject {
        project_name: String,
        working_dir: String,
    },
    DeleteChat(ChatId),
    OpenPopup(Popup),
    ClosePopup,
    CopyPopupContent,
    EditShortText(String),
    EditLongText(TextEditorAction),
    FocusLongTextEdit,
    SetProjectConfig(SetProjectConfig),
    SetChatConfig(SetChatConfig),
    UpdateModelStore(ModelStoreMessage),
    MainViewScrolled(AbsoluteOffset),
    BackgroundJobResult(JobResult),
    Focus,
    PrepareScratchPad,
    OpenScratchPad { title: Option<String>, content: ScratchPadContent },
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { unreachable!() }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
    fn open_scratch_pad() -> Self { IcedMessage::PrepareScratchPad }
}

#[derive(Clone, Debug)]
pub enum Popup {
    NewProject,
    ProjectConfig,
    NewChat,
    ChatConfig,
    Instruction {
        working_dir: String,
    },
    AskDeleteProject {
        project_name: String,
        working_dir: String,
    },
    AskDeleteChat {
        id: ChatId,
        title: Option<String>,
    },
    ModelStore,
    Help,
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
        IcedMessage::Tick { frame, force_update } => {
            context.now = Local::now().to_rfc2822();

            if frame % 8 == 0 || force_update {
                context.recent_projects = load_all_indexes(&context.global_index_dir);
                context.recent_chats = load_all_chats(&context.global_index_dir)?;

                // let's just assume that there's no overflow!
                context.recent_projects.sort_by_key(|p| -p.updated_at);
                context.recent_chats.sort_by_key(|c| -c.updated_at);

                context.update_battery_state();
            }

            context.working_dir_tab_indexes = context.current_tabs.iter().enumerate().filter_map(
                |(i, tab)| tab.neukgu_id.map(|id| (id, i))
            ).collect();

        },
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
            (Key::Named(NamedKey::Escape), false, false, false) => {
                if context.can_close_popup() {
                    return Ok(Task::done(IcedMessage::ClosePopup));
                }
            },
            (Key::Named(NamedKey::ArrowUp), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(snap_to(context.main_view_id.clone(), RelativeOffset { x: 0.0, y: 0.0 }));
                }

                else {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 0.0 }));
                }
            },
            (Key::Named(NamedKey::ArrowDown), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(snap_to(context.main_view_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }));
                }

                else {
                    return Ok(snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 1.0 }));
                }
            },
            (Key::Character("c"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::NewChat)));
                }
            },
            (Key::Character("h"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::Help)));
                }
            },
            (Key::Character("m"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::ModelStore)));
                }
            },
            (Key::Character("p"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup(Popup::NewProject)));
                }
            },
            // tabs::update will do the exact same thing with the exact same key binding
            // (Key::Character("t"), true, false, false) => {
            //     if context.curr_popup.is_none() {
            //         return Ok(Task::done(IcedMessage::NewTab { tab: Tab::Browser { dir: context.home_dir.to_string(), file: None }, force_new_tab: true }));
            //     }
            // },
            (Key::Character("y"), true, false, false) => {
                if let Some(Popup::AskDeleteProject { project_name, working_dir }) = &context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteProject { project_name: project_name.to_string(), working_dir: working_dir.to_string() }));
                }

                else if let Some(Popup::AskDeleteChat { id, .. }) = &context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteChat(*id)));
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
        IcedMessage::NewTab { .. } => unreachable!(),
        IcedMessage::OpenTab { .. } => unreachable!(),
        IcedMessage::ExpandProjectSection => {
            context.project_section_expanded = !context.project_section_expanded;
        },
        IcedMessage::ExpandChatSection => {
            context.chat_section_expanded = !context.chat_section_expanded;
        },
        IcedMessage::ExpandTabSection => {
            context.tab_section_expanded = !context.tab_section_expanded;
        },
        IcedMessage::NewProject => {
            let project_name = context.short_text_editor_content.to_string();
            validate_project_name(&project_name)?;
            let instruction = context.long_text_editor_content.text();
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;
            create_dir(&project_path)?;
            init_working_dir(Some(instruction), &project_path, context.new_project_config.clone(), true)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::NewTab { tab: Tab::WorkingDir(project_path), force_new_tab: true }));
        },
        IcedMessage::SaveGlobalProjectConfig => {
            save_global_config(&context.new_project_config, &context.global_index_dir)?;
            context.close_popup();
        },
        IcedMessage::NewChat => {
            let chat_title = context.short_text_editor_content.to_string();
            let chat_title = if chat_title.is_empty() { None } else { Some(chat_title) };
            let chat_id = init_chat(chat_title, context.new_chat_config.clone(), &context.global_index_dir)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::NewTab { tab: Tab::Chat(chat_id), force_new_tab: true }));
        },
        IcedMessage::SaveGlobalChatConfig => {
            save_global_chat_config(&context.new_chat_config, &context.global_index_dir)?;
            context.close_popup();
        },
        IcedMessage::DeleteProject { project_name, working_dir } => {
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;
            let neukgu_id = get_neukgu_id(&working_dir)?;
            remove_dir_all(&project_path)?;
            remove_global_index(&context.global_index_dir, neukgu_id)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
        },
        IcedMessage::DeleteChat(chat_id) => {
            delete_chat(chat_id, &context.global_index_dir)?;
            context.close_popup();
            return Ok(Task::done(IcedMessage::Tick { frame: 0, force_update: true }));
        },
        IcedMessage::OpenPopup(popup) => {
            return context.open_popup(popup);
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
        },
        IcedMessage::CopyPopupContent => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::EditShortText(s) => {
            context.short_text_editor_content = s;
        },
        IcedMessage::EditLongText(a) => {
            context.long_text_editor_content.perform(a);
        },
        IcedMessage::FocusLongTextEdit => {
            return Ok(focus(context.long_text_editor_id.clone()));
        },
        IcedMessage::SetProjectConfig(c) => {
            set_project_config(&mut context.new_project_config, c);
        },
        IcedMessage::SetChatConfig(c) => {
            set_chat_config(&mut context.new_chat_config, c);
        },
        IcedMessage::UpdateModelStore(m) => {
            return Ok(model_store::update(&mut context.model_store_context, m)?.map(IcedMessage::UpdateModelStore));
        },
        IcedMessage::MainViewScrolled(o) => {
            context.main_view_scrolled = o;
        },
        IcedMessage::BackgroundJobResult(_) => todo!(),
        IcedMessage::Focus => {
            context.hovered_tab = None;
            return Ok(scroll_to(context.main_view_id.clone(), context.main_view_scrolled));
        },
        IcedMessage::PrepareScratchPad => {
            return Ok(Task::done(IcedMessage::OpenScratchPad {
                title: None,
                content: ScratchPadContent::Text { content: context.copy_buffer.clone().unwrap(), extension: context.syntax_highlight.clone() },
            }));
        },
        IcedMessage::OpenScratchPad { .. } => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    fn section<'c>(
        title: &'static str,
        buttons: Vec<Button<'c, IcedMessage>>,
        is_expanded: bool,
        expand_message: IcedMessage,
        entries: Element<'c, IcedMessage>,
        context: &'c IcedContext,
    ) -> Column<'c, IcedMessage> {
        let expand_button: Button<IcedMessage> = match (context.curr_popup.is_some(), is_expanded) {
            (true, true) => disabled_button("▼", white(), context.zoom),
            (false, true) => button("▼", expand_message, white(), context.zoom),
            (true, false) => disabled_button("▶", white(), context.zoom),
            (false, false) => button("▶", expand_message, white(), context.zoom),
        };
        let buttons: Vec<Element<IcedMessage>> = if context.curr_popup.is_some() {
            buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
        } else {
            buttons.into_iter().map(|button| button.into()).collect()
        };

        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{title}").size(context.zoom * 14.0).into(),
                Row::from_vec(buttons).spacing(context.zoom * 8.0).into(),
            ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into(),
            Row::from_vec(vec![
                expand_button.into(),
                Container::new(Scrollable::new(entries).width(Length::Fill))
                    .style(|_| set_round_bg(black(), context.zoom))
                    .padding(context.zoom * 8.0)
                    .width(context.window_size.width)
                    .height(if is_expanded { context.window_size.height * 0.6 } else { context.window_size.height * 0.2 })
                    .into(),
            ]).spacing(context.zoom * 8.0).into(),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
    }

    let c = Column::from_vec(vec![
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{}", context.now).size(context.zoom * 14.0).into(),
                render_battery_state(context),
            ])
                .spacing(context.zoom * 8.0)
                .into(),
            render_buttons(context),
        ])
            .padding(context.zoom * 8.0)
            .spacing(context.zoom * 8.0)
            .into(),
        section(
            "Recent projects",
            vec![
                button(
                    "New (p)roject",
                    IcedMessage::OpenPopup(Popup::NewProject),
                    green(),
                    context.zoom,
                ),
                button(
                    "Config",
                    IcedMessage::OpenPopup(Popup::ProjectConfig),
                    blue(),
                    context.zoom,
                ),
            ],
            context.project_section_expanded,
            IcedMessage::ExpandProjectSection,
            render_projects(context),
            context,
        ).into(),
        section(
            "Recent chats",
            vec![
                button(
                    "New (c)hat",
                    IcedMessage::OpenPopup(Popup::NewChat),
                    brown(),
                    context.zoom,
                ),
                button(
                    "Config",
                    IcedMessage::OpenPopup(Popup::ChatConfig),
                    blue(),
                    context.zoom,
                ),
            ],
            context.chat_section_expanded,
            IcedMessage::ExpandChatSection,
            render_chats(context),
            context,
        ).into(),
        section(
            "Current tabs",
            vec![button(
                "New (t)ab",
                IcedMessage::NewTab {
                    tab: Tab::Browser { dir: context.home_dir.to_string(), file: None },
                    force_new_tab: true,
                },
                skyblue(),
                context.zoom,
            )],
            context.tab_section_expanded,
            IcedMessage::ExpandTabSection,
            render_tabs(context),
            context,
        ).into(),
    ]).spacing(context.zoom * 8.0);

    let mut full_view_stacked: Element<IcedMessage> = Scrollable::new(c)
        .id(context.main_view_id.clone())
        .on_scroll(|v| IcedMessage::MainViewScrolled(v.absolute_offset()))
        .into();

    if let Some(Popup::NewProject) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_new_project_popup(context),
        ]).into();
    }

    else if let Some(Popup::ProjectConfig) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(
                Column::from_vec(vec![
                    config_ui(&context.new_project_config, context.zoom).map(IcedMessage::SetProjectConfig),
                    button("Save", IcedMessage::SaveGlobalProjectConfig, green(), context.zoom).into(),
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
            ).id(context.popup_scroll_id.clone()).into(), context),
        ]).into();
    }

    else if let Some(Popup::NewChat) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_new_chat_popup(context),
        ]).into();
    }

    else if let Some(Popup::ChatConfig) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(
                Column::from_vec(vec![
                    chat_config_ui1(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig),
                    chat_config_ui2(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig),
                    button("Save", IcedMessage::SaveGlobalChatConfig, green(), context.zoom).into(),
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
            ).id(context.popup_scroll_id.clone()).into(), context),
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

    else if let Some(Popup::AskDeleteProject { project_name, working_dir }) = &context.curr_popup {
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

    else if let Some(Popup::AskDeleteChat { id, title }) = &context.curr_popup {
        let ask = Column::from_vec(vec![
            if let Some(title) = title {
                text!("Delete chat `{title}`?").size(context.zoom * 14.0).into()
            } else {
                text!("Delete untitled chat?").size(context.zoom * 14.0).into()
            },
            button("(Y)es", IcedMessage::DeleteChat(*id), green(), context.zoom).into(),
        ])
            .spacing(context.zoom * 20.0)
            .align_x(Horizontal::Center)
            .width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(ask.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::ModelStore) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(
                model_store::view(
                    &context.model_store_context,
                    context.popup_scroll_id.clone(),
                    context.zoom,
                ).map(IcedMessage::UpdateModelStore),
                context,
            ).into(),
        ]).into();
    }

    else if let Some(Popup::Instruction { .. } | Popup::Help) = context.curr_popup {
        let text_editor = TextEditor::new(&context.long_text_editor_content).size(context.zoom * 14.0).highlight(
            &if let Some(extension) = &context.syntax_highlight { extension.to_string() } else { String::from("txt") },
            iced::highlighter::Theme::SolarizedDark,
        );

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(text_editor).id(context.popup_scroll_id.clone()).width(Length::Fill).into(), context),
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut buttons = vec![
        button("(M)odel Store", IcedMessage::OpenPopup(Popup::ModelStore), blue(), context.zoom),
        button("(H)elp", IcedMessage::OpenPopup(Popup::Help), pink(), context.zoom),
    ];

    if context.curr_popup.is_some() {
        buttons = buttons.into_iter().map(|button| button.on_press_maybe(None)).collect();
    }

    Row::from_vec(buttons.into_iter().map(|button| button.into()).collect()).spacing(context.zoom * 8.0).into()
}

fn render_projects<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let column: Vec<Element<IcedMessage>> = context.recent_projects.iter().map(
        |project| {
            let path = if project.is_in_global_index_dir {
                basename(&project.working_dir).unwrap()
            } else {
                project.working_dir.to_string()
            };
            let elapsed = if project.error.is_some() {
                String::new()
            } else {
                format!("({})", prettify_timestamp(project.updated_at))
            };

            Container::new(
                Column::from_vec(vec![
                    text!("{path} {elapsed}").size(context.zoom * 14.0).width(context.window_size.width).into(),
                    if let Some(error) = &project.error {
                        text!("{error}").size(context.zoom * 14.0).color(red()).into()
                    } else if context.curr_popup.is_some() {
                        Row::from_vec(vec![
                            if context.working_dir_tab_indexes.contains_key(&project.neukgu_id) {
                                disabled_button("Switch", green(), context.zoom).into()
                            } else {
                                disabled_button("Launch", green(), context.zoom).into()
                            },
                            disabled_button("Browse", skyblue(), context.zoom).into(),
                            disabled_button("Instruction", yellow(), context.zoom).into(),
                            if project.is_in_global_index_dir {
                                disabled_button("Delete", red(), context.zoom).into()
                            } else {
                                Space::new().into()
                            },
                        ]).spacing(context.zoom * 4.0).into()
                    } else {
                        // TODO: see summaries of the session
                        Row::from_vec(vec![
                            if context.working_dir_tab_indexes.contains_key(&project.neukgu_id) {
                                button("Switch", IcedMessage::NewTab { tab: Tab::WorkingDir(project.working_dir.to_string()), force_new_tab: false }, green(), context.zoom).into()
                            } else {
                                button("Launch", IcedMessage::NewTab { tab: Tab::WorkingDir(project.working_dir.to_string()), force_new_tab: false }, green(), context.zoom).into()
                            },
                            button("Browse", IcedMessage::NewTab { tab: Tab::Browser { dir: project.working_dir.to_string(), file: None }, force_new_tab: false }, skyblue(), context.zoom).into(),
                            button("Instruction", IcedMessage::OpenPopup(Popup::Instruction { working_dir: project.working_dir.to_string() }), yellow(), context.zoom).into(),
                            if project.is_in_global_index_dir {
                                button("Delete", IcedMessage::OpenPopup(Popup::AskDeleteProject {
                                    project_name: path.to_string(),
                                    working_dir: project.working_dir.to_string(),
                                }), red(), context.zoom).into()
                            } else {
                                Space::new().into()
                            },
                        ]).spacing(context.zoom * 4.0).into()
                    },
                ]).spacing(context.zoom * 4.0)
            )
                .style(|_| set_round_bg(gray(0.3), context.zoom))
                .padding(context.zoom * 8.0)
                .into()
        }
    ).collect();
    Column::from_vec(column).spacing(context.zoom * 8.0).into()
}

fn render_chats<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let column: Vec<Element<IcedMessage>> = context.recent_chats.iter().map(
        |chat| Container::new(
            Column::from_vec(vec![
                text!(
                    "{} ({})",
                    chat.title.as_ref().unwrap_or(&String::from("untitled")),
                    prettify_timestamp(chat.updated_at),
                ).size(context.zoom * 14.0).width(context.window_size.width).into(),
                Row::from_vec(
                    if context.curr_popup.is_some() {
                        vec![
                            disabled_button("Open", green(), context.zoom).into(),
                            disabled_button("Delete", red(), context.zoom).into(),
                        ]
                    } else {
                        vec![
                            button("Open", IcedMessage::NewTab { tab: Tab::Chat(chat.id), force_new_tab: false }, green(), context.zoom).into(),
                            button("Delete", IcedMessage::OpenPopup(Popup::AskDeleteChat { id: chat.id, title: chat.title.clone() }), red(), context.zoom).into(),
                        ]
                    }
                ).spacing(context.zoom * 8.0).into(),
            ]).spacing(context.zoom * 8.0)
        )
            .style(|_| set_round_bg(gray(0.3), context.zoom))
            .padding(context.zoom * 8.0)
            .into()
    ).collect();

    Column::from_vec(column).spacing(context.zoom * 8.0).into()
}

fn render_tabs<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    fn draw_bg(e: Row<IcedMessage>, is_hovered: bool, window_width: f32, zoom: f32) -> Container<IcedMessage> {
        Container::new(e)
            .style(move |_| set_round_bg(if is_hovered { gray(0.45) } else { gray(0.15) }, zoom))
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

fn render_new_project_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
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
                config_ui(&context.new_project_config, context.zoom).map(IcedMessage::SetProjectConfig).into(),
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

fn render_new_chat_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let short_text_editor = TextInput::new("(untitled)", &context.short_text_editor_content)
        .size(context.zoom * 14.0)
        .id(context.short_text_editor_id.clone())
        .on_input(|input| IcedMessage::EditShortText(input));

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                chat_config_ui1(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig).into(),
                chat_config_ui2(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig).into(),
                button("Create", IcedMessage::NewChat, brown(), context.zoom).into(),
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

fn render_battery_state<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    fn cell<'c>(on: bool, color: Color, zoom: f32) -> Element<'c, IcedMessage> {
        let mut cell = Container::new(text!(" ").size(zoom * 14.0));
        let background = if on {
            Some(Background::Color(color))
        } else {
            None
        };

        if on {
            cell = cell.style(move |_| Style {
                background,
                border: Border {
                    color: white(),
                    width: 0.0,
                    radius: Radius::new(zoom * 2.0),
                },
                ..Style::default()
            });
        }

        cell.into()
    }

    match context.battery {
        Some((state, charged)) => {
            let cell_color = if charged < 0.333 {
                red()
            } else if charged < 0.667 {
                yellow()
            } else {
                green()
            };

            let battery = Container::new(
                Row::from_vec(vec![
                    cell(charged > 0.0, cell_color, context.zoom),
                    cell(charged > 0.143, cell_color, context.zoom),
                    cell(charged > 0.286, cell_color, context.zoom),
                    cell(charged > 0.429, cell_color, context.zoom),
                    cell(charged > 0.571, cell_color, context.zoom),
                    cell(charged > 0.714, cell_color, context.zoom),
                    cell(charged > 0.857, cell_color, context.zoom),
                ])
            ).style(move |_| Style {
                background: None,
                border: Border {
                    color: white(),
                    width: context.zoom * 4.0,
                    radius: Radius::new(context.zoom * 6.0),
                },
                ..Style::default()
            }).padding(context.zoom * 2.0);

            Row::from_vec(vec![
                battery.into(),
                if state == battery::State::Charging {
                    circle(context.zoom * 4.0, green())
                } else {
                    Space::new().into()
                },
            ])
                .align_y(Vertical::Center)
                .spacing(context.zoom * 4.0)
                .into()
        },
        None => Space::new().into(),
    }
}
