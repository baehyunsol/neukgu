use super::{
    TEXT_EDITOR_CONTENT_LIMIT,
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
    set_bg,
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
    chat_config_ui3,
    config_ui,
    set_chat_config,
    set_project_config,
};
use super::file_selector::{self, IcedContext as FileSelectorContext, IcedMessage as FileSelectorMessage};
use super::model_store::{self, IcedContext as ModelStoreContext, IcedMessage as ModelStoreMessage};
use super::popup::{PopupContext, PopupMessage, into_popup};
use super::scratch_pad::Content as ScratchPadContent;
use super::tab::{TabId, TabPreview};
use super::tabs::Tab;
use super::worker::{Job, JobId, JobKind, JobResult, JobResultKind};
use chrono::Local;
use crate::{
    Chat,
    ChatId,
    Config,
    Error,
    MatchPreview,
    NeukguId,
    Project,
    ProjectJson,
    Skill,
    SkillSchemaError,
    build_info,
    clean_global_index_dir,
    delete_chat,
    get_chat_system_prompts,
    get_global_chat_config,
    get_global_config,
    get_global_index_dir,
    get_neukgu_id,
    init_chat,
    init_global_index_dir,
    init_working_dir,
    load_all_chats,
    load_all_indexes,
    load_global_skills,
    load_json,
    prettify_timestamp,
    remove_global_index,
    save_chat_system_prompts,
    save_global_chat_config,
    save_global_config,
    truncate_chars,
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
    exists,
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
- Ctrl+Shift+C: open calendar
- Ctrl+Shift+E: expand/collapse scratch pad
- Ctrl+Shift+M: open memo
- Ctrl+Shift+P: open slide rule
- Ctrl+Shift+Left/Right: move scratch pad
- Ctrl+Shift+Plus/Minus: zoom
"#;

pub struct IcedContext {
    pub home_dir: String,
    pub global_index_dir: String,
    pub window_size: Size,
    pub recent_projects: Vec<Project>,
    pub recent_chats: Vec<Chat>,

    // This is synchronized with `<global-index-dir>/skills/` and is read-only.
    pub skills: Vec<(String, Result<Skill, SkillSchemaError>)>,

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
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub syntax_highlight: Option<String>,

    pub new_project_config: Config,
    pub new_chat_config: ChatConfig,
    pub system_prompts: Vec<String>,
    pub model_store_context: ModelStoreContext,
    pub file_selector_context: Option<FileSelectorContext>,
    pub short_text_editor_content: String,

    // For `Popup::Skill { .. }`, it uses long_text_editor
    // to edit the description and extra_text_editor to edit the body.
    pub long_text_editor_content: TextEditorContent,
    pub extra_text_editor_content: TextEditorContent,
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
            skills: load_global_skills(&global_index_dir).unwrap(),
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
            prev_popup: None,
            copy_buffer: None,
            syntax_highlight: None,
            new_project_config: get_global_config(&global_index_dir).unwrap(),
            new_chat_config: get_global_chat_config(&global_index_dir).unwrap(),
            system_prompts: get_chat_system_prompts(&global_index_dir).unwrap(),
            model_store_context: ModelStoreContext::new(global_index_dir.to_string()),
            file_selector_context: None,
            short_text_editor_content: String::new(),
            long_text_editor_content: TextEditorContent::new(),
            extra_text_editor_content: TextEditorContent::new(),
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
            Popup::ExistingProjectConfig(id) => {
                let ProjectJson { working_dir, .. } = load_json(&join3(&self.global_index_dir, "indexes", &format!("{:016x}", id.0))?)?;
                self.new_project_config = Config::load(&working_dir)?;
            },
            Popup::Skills => {},
            Popup::Skill { name } => match name {
                Some(skill_name) => {
                    let mut skill = None;

                    for (n, s) in self.skills.iter() {
                        if n == &skill_name {
                            skill = Some(s.clone().unwrap());
                        }
                    }

                    // Otherwise, the ui should have rejected.
                    let skill = skill.unwrap();

                    self.short_text_editor_content = skill_name.to_string();
                    self.set_long_text_editor_content(skill.description.to_string());
                    self.set_extra_text_editor_content(skill.body.to_string());
                },
                None => {
                    self.short_text_editor_content = String::new();
                    self.set_long_text_editor_content(String::new());
                    self.set_extra_text_editor_content(String::new());
                },
            },
            Popup::NewChat => {
                self.new_chat_config = get_global_chat_config(&self.global_index_dir)?;
            },
            Popup::ChatConfig => {
                self.new_chat_config = get_global_chat_config(&self.global_index_dir)?;
            },
            Popup::ExistingChatConfig(id) => {
                let Chat { config, .. } = Chat::load(id, &self.global_index_dir)?;
                self.new_chat_config = config;
            },
            Popup::ChatSystemPrompts => {},
            Popup::EditChatSystemPrompt(i) => {
                self.set_long_text_editor_content(self.system_prompts[i].to_string());
            },
            Popup::Instruction { working_dir } => {
                let instruction = read_string(&join(&working_dir, "neukgu-instruction.md")?)?;
                self.copy_buffer = Some(instruction.to_string());
                self.set_long_text_editor_content(instruction);
                self.syntax_highlight = Some(String::from("md"));
            },
            Popup::AskDeleteProject { .. } => {},
            Popup::AskDeleteSkill { .. } => {},
            Popup::AskDeleteChat { .. } => {},
            Popup::AskDeleteChatSystemPrompt(_) => {},
            Popup::FindInChats { .. } => {},
            Popup::FindInChatsResult { .. } => {},
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
        self.file_selector_context = None;
    }

    pub fn copy_attached_files(&self, project_path: &str) -> Result<(), Error> {
        if let Some(c) = &self.file_selector_context {
            c.copy_selected_files(project_path)?;
        }

        Ok(())
    }

    pub fn set_long_text_editor_content(&mut self, c: String) {
        self.long_text_editor_content.perform(TextEditorAction::SelectAll);
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.long_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn set_extra_text_editor_content(&mut self, c: String) {
        self.extra_text_editor_content.perform(TextEditorAction::SelectAll);
        self.extra_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.extra_text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    pub fn zoom_in(&mut self) -> Task<IcedMessage> {
        if self.zoom < 2.4 {
            self.zoom += 0.1;
            Task::none()
        } else {
            Task::done(IcedMessage::Notify(String::from("Cannot zoom in anymore.")))
        }
    }

    pub fn zoom_out(&mut self) -> Task<IcedMessage> {
        if self.zoom > 0.4 {
            self.zoom -= 0.1;
            Task::none()
        } else {
            Task::done(IcedMessage::Notify(String::from("Cannot zoom out anymore.")))
        }
    }
}

impl PopupContext for IcedContext {
    fn can_close_popup(&self) -> bool { self.curr_popup.is_some() }
    fn has_prev_popup(&self) -> bool { self.prev_popup.is_some() }
    fn has_something_to_copy(&self) -> bool { self.copy_buffer.is_some() }

    fn can_open_scratch_pad(&self) -> bool {
        if let Some(c) = &self.copy_buffer && c.len() < TEXT_EDITOR_CONTENT_LIMIT {
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
    SaveProjectConfig(NeukguId),
    NewChat,

    // It might change the name of the skill, so it remembers the old name.
    UpdateSkill { name: String },

    CreateSkill,
    SaveGlobalChatConfig,
    SaveChatConfig(ChatId),
    DeleteProject {
        project_name: String,
        working_dir: String,
    },
    DeleteSkill(String),
    DeleteChat(ChatId),
    AddChatSystemPrompt,
    EditChatSystemPrompt(usize),
    DeleteChatSystemPrompt(usize),
    OpenPopup {
        curr: Popup,
        prev: Option<Popup>,
    },
    BackPopup,
    ClosePopup,
    CopyPopupContent,
    FindInChats,
    EditShortText(String),
    EditLongText(TextEditorAction),
    EditExtraText(TextEditorAction),
    FocusLongTextEdit,
    SetProjectConfig(SetProjectConfig),
    SetChatConfig(SetChatConfig),
    UpdateModelStore(ModelStoreMessage),
    OpenFileSelector,
    FileSelectorMessage(FileSelectorMessage),
    MainViewScrolled(AbsoluteOffset),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Notify(String),
    Focus,
    PrepareScratchPad,
    OpenScratchPad { title: Option<String>, content: ScratchPadContent },
}

impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { IcedMessage::ClosePopup }
    fn back_popup() -> Self { IcedMessage::BackPopup }
    fn copy_popup_content() -> Self { IcedMessage::CopyPopupContent }
    fn open_scratch_pad() -> Self { IcedMessage::PrepareScratchPad }
}

#[derive(Clone, Debug)]
pub enum Popup {
    NewProject,
    ProjectConfig,
    ExistingProjectConfig(NeukguId),
    Skills,
    Skill {
        // If it's None, it's a new skill.
        name: Option<String>,
    },
    NewChat,
    ChatConfig,
    ExistingChatConfig(ChatId),
    ChatSystemPrompts,
    EditChatSystemPrompt(usize),
    Instruction {
        working_dir: String,
    },
    AskDeleteProject {
        project_name: String,
        working_dir: String,
    },
    AskDeleteSkill(String),
    AskDeleteChat {
        id: ChatId,
        title: Option<String>,
    },
    AskDeleteChatSystemPrompt(usize),
    FindInChats {
        // It'll be set when the background worker starts working.
        job_id: Option<JobId>,

        error: Option<String>,
    },
    FindInChatsResult {
        regex: String,
        matches: Vec<(ChatId, Vec<MatchPreview>)>,
    },
    ModelStore,
    Help,
    Error(String),
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::OpenPopup { curr: Popup::Error(format!("{e:?}")), prev: None }),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick { frame, force_update } => {
            context.now = Local::now().to_rfc2822();

            if frame % 8 == 0 || force_update {
                context.recent_projects = load_all_indexes(&context.global_index_dir);
                context.recent_chats = load_all_chats(&context.global_index_dir)?;
                context.skills = load_global_skills(&context.global_index_dir)?;

                // let's just assume that there's no overflow!
                context.recent_projects.sort_by_key(|p| -p.updated_at);
                context.recent_chats.sort_by_key(|c| -c.updated_at);
                context.skills.sort_by_key(|(n, _)| n.to_string());

                context.system_prompts = get_chat_system_prompts(&context.global_index_dir)?;
                context.update_battery_state();
            }

            context.working_dir_tab_indexes = context.current_tabs.iter().enumerate().filter_map(
                |(i, tab)| tab.neukgu_id.map(|id| (id, i))
            ).collect();

            if let Some(c) = &mut context.file_selector_context {
                return Ok(file_selector::update(c, FileSelectorMessage::Tick { frame }).map(IcedMessage::FileSelectorMessage));
            }
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
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::NewChat, prev: None }));
                }
            },
            (Key::Character("h"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::Help, prev: None }));
                }
            },
            (Key::Character("m"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::ModelStore, prev: None }));
                }
            },
            (Key::Character("p"), true, false, false) => {
                if context.curr_popup.is_none() {
                    return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::NewProject, prev: None }));
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

                if let Some(Popup::AskDeleteSkill(name)) = &context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteSkill(name.to_string())));
                }

                else if let Some(Popup::AskDeleteChat { id, .. }) = &context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteChat(*id)));
                }

                else if let Some(Popup::AskDeleteChatSystemPrompt(i)) = context.curr_popup {
                    return Ok(Task::done(IcedMessage::DeleteChatSystemPrompt(i)));
                }
            },
            (Key::Character("-"), true, false, false) => {
                return Ok(context.zoom_out());
            },
            (Key::Character("="), true, false, false) => {
                return Ok(context.zoom_in());
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

            if let Err(e) = validate_project_name(&project_name) {
                return Ok(Task::done(IcedMessage::Notify(format!("Error: {e:?}"))));
            }

            let instruction = context.long_text_editor_content.text();
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;

            if exists(&project_path) {
                return Ok(Task::done(IcedMessage::Notify(format!("Project `{project_name}` already exists!"))));
            }

            create_dir(&project_path)?;
            context.copy_attached_files(&project_path)?;
            context.file_selector_context = None;
            init_working_dir(Some(instruction), &project_path, context.new_project_config.clone(), Some(join(&context.global_index_dir, "skills")?), true)?;
            return Ok(Task::batch(vec![
                Task::done(IcedMessage::NewTab { tab: Tab::WorkingDir(project_path), force_new_tab: true }),
                Task::done(IcedMessage::ClosePopup),
            ]));
        },
        IcedMessage::SaveGlobalProjectConfig => {
            save_global_config(&context.new_project_config, &context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::ClosePopup));
        },
        IcedMessage::SaveProjectConfig(id) => {
            let ProjectJson { working_dir, .. } = load_json(&join3(&context.global_index_dir, "indexes", &format!("{:016x}", id.0))?)?;
            context.new_project_config.store(&working_dir)?;
            return Ok(Task::done(IcedMessage::ClosePopup));
        },
        IcedMessage::NewChat => {
            let chat_title = context.short_text_editor_content.to_string();
            let chat_title = if chat_title.is_empty() { None } else { Some(chat_title) };
            let chat_id = init_chat(chat_title, context.new_chat_config.clone(), &context.global_index_dir)?;
            return Ok(Task::batch(vec![
                Task::done(IcedMessage::NewTab { tab: Tab::Chat(chat_id), force_new_tab: true }),
                Task::done(IcedMessage::ClosePopup),
            ]));
        },
        IcedMessage::UpdateSkill { name: old_name } => {
            let new_name = context.short_text_editor_content.to_string();
            let description = context.long_text_editor_content.text();
            let body = context.extra_text_editor_content.text();
            context.new_project_config = get_global_config(&context.global_index_dir)?;

            if old_name != new_name && context.new_project_config.skills.contains_key(&new_name) {
                return Ok(Task::done(IcedMessage::Notify(format!("Skill `{new_name}` already exists!"))));
            }

            let skill = match Skill::try_init(new_name.clone(), description, None, None, body) {
                Ok(skill) => skill,
                Err(e) => {
                    return Ok(Task::done(IcedMessage::Notify(format!("Cannot update skill: {e}"))));
                },
            };

            let old_skill_path = join3(&context.global_index_dir, "skills", &old_name)?;
            let new_skill_path = join3(&context.global_index_dir, "skills", &new_name)?;

            if old_name != new_name {
                ragit_fs::rename(&old_skill_path, &new_skill_path)?;
            }

            skill.save(&context.global_index_dir)?;
            context.new_project_config.remove_skill(&old_name);
            context.new_project_config.add_skill(skill);
            return Ok(Task::done(IcedMessage::SaveGlobalProjectConfig));
        },
        IcedMessage::CreateSkill => {
            let name = context.short_text_editor_content.to_string();
            let description = context.long_text_editor_content.text();
            let body = context.extra_text_editor_content.text();

            match Skill::try_init(name, description, None, None, body) {
                Ok(skill) => {
                    context.new_project_config = get_global_config(&context.global_index_dir)?;

                    if context.new_project_config.skills.contains_key(&skill.name) {
                        return Ok(Task::done(IcedMessage::Notify(format!("Skill `{}` already exists!", skill.name))));
                    }

                    skill.save(&context.global_index_dir)?;
                    context.new_project_config.add_skill(skill);
                    return Ok(Task::done(IcedMessage::SaveGlobalProjectConfig));
                },
                Err(e) => {
                    return Ok(Task::done(IcedMessage::Notify(format!("Cannot create skill: {e}"))));
                },
            }
        },
        IcedMessage::SaveGlobalChatConfig => {
            save_global_chat_config(&context.new_chat_config, &context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::ClosePopup));
        },
        IcedMessage::SaveChatConfig(id) => {
            let mut chat = Chat::load(id, &context.global_index_dir)?;
            chat.config = context.new_chat_config.clone();
            chat.store(&context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::ClosePopup));
        },
        IcedMessage::DeleteProject { project_name, working_dir } => {
            let project_path = join3(&context.global_index_dir, "projects", &project_name)?;
            let neukgu_id = get_neukgu_id(&working_dir)?;
            remove_dir_all(&project_path)?;
            remove_global_index(&context.global_index_dir, neukgu_id)?;
            return Ok(Task::batch(vec![
                Task::done(IcedMessage::Tick { frame: 0, force_update: true }),
                Task::done(IcedMessage::ClosePopup),
            ]));
        },
        IcedMessage::DeleteSkill(name) => {
            let skill_path = join3(&context.global_index_dir, "skills", &name)?;
            remove_dir_all(&skill_path)?;
            context.new_project_config = get_global_config(&context.global_index_dir)?;
            context.new_project_config.remove_skill(&name);
            return Ok(Task::done(IcedMessage::SaveGlobalProjectConfig));
        },
        IcedMessage::DeleteChat(chat_id) => {
            delete_chat(chat_id, &context.global_index_dir)?;
            return Ok(Task::batch(vec![
                Task::done(IcedMessage::Tick { frame: 0, force_update: true }),
                Task::done(IcedMessage::ClosePopup),
            ]));
        },
        IcedMessage::AddChatSystemPrompt => {
            context.system_prompts.push(String::new());
            save_chat_system_prompts(&context.system_prompts, &context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::OpenPopup {
                curr: Popup::EditChatSystemPrompt(context.system_prompts.len() - 1),
                prev: Some(Popup::ChatSystemPrompts),
            }));
        },
        IcedMessage::EditChatSystemPrompt(i) => {
            context.system_prompts[i] = context.long_text_editor_content.text();
            save_chat_system_prompts(&context.system_prompts, &context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::ChatSystemPrompts, prev: None }));
        },
        IcedMessage::DeleteChatSystemPrompt(i) => {
            context.system_prompts.remove(i);
            save_chat_system_prompts(&context.system_prompts, &context.global_index_dir)?;
            return Ok(Task::done(IcedMessage::OpenPopup { curr: Popup::ChatSystemPrompts, prev: None }));
        },
        IcedMessage::OpenPopup { curr, prev } => {
            let mut tasks: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.main_view_id.clone(), context.main_view_scrolled),
            ];

            match &curr {
                Popup::EditChatSystemPrompt(_) => {
                    tasks.push(focus(context.long_text_editor_id.clone()));
                },
                Popup::NewProject | Popup::Skill { .. } | Popup::NewChat | Popup::FindInChats { .. } => {
                    tasks.push(focus(context.short_text_editor_id.clone()));
                },
                _ => {},
            }

            tasks.push(context.open_popup(curr)?);
            context.prev_popup = prev;
            return Ok(Task::batch(tasks));
        },
        IcedMessage::BackPopup => {
            if let Some(prev_popup) = &context.prev_popup {
                let prev_popup = prev_popup.clone();
                context.prev_popup = None;
                return Ok(context.open_popup(prev_popup)?);
            }
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.main_view_id.clone(), context.main_view_scrolled));
        },
        IcedMessage::CopyPopupContent => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::FindInChats => {
            let new_job_id = JobId::new();

            if let Some(Popup::FindInChats { job_id, error }) = &mut context.curr_popup {
                *job_id = Some(new_job_id);
                *error = None;
            }

            return Ok(Task::done(IcedMessage::BackgroundJob(Job {
                id: new_job_id,
                kind: JobKind::FindInChats {
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
        IcedMessage::EditExtraText(a) => {
            context.extra_text_editor_content.perform(a);
        },
        IcedMessage::FocusLongTextEdit => {
            return Ok(focus(context.long_text_editor_id.clone()));
        },
        IcedMessage::SetProjectConfig(c) => {
            set_project_config(&mut context.new_project_config, c);
        },
        IcedMessage::SetChatConfig(c) => {
            set_chat_config(&mut context.new_chat_config, &context.system_prompts, c);
        },
        IcedMessage::UpdateModelStore(m) => {
            return Ok(model_store::update(&mut context.model_store_context, m)?.map(IcedMessage::UpdateModelStore));
        },
        IcedMessage::OpenFileSelector => {
            context.file_selector_context = Some(FileSelectorContext::new(context.home_dir.to_string()));
        },
        IcedMessage::FileSelectorMessage(FileSelectorMessage::BackgroundJob(j)) => {
            return Ok(Task::done(IcedMessage::BackgroundJob(j)));
        },
        IcedMessage::FileSelectorMessage(FileSelectorMessage::Notify(m)) => {
            return Ok(Task::done(IcedMessage::Notify(m)));
        },
        IcedMessage::FileSelectorMessage(m) => {
            if let Some(c) = &mut context.file_selector_context {
                return Ok(file_selector::update(c, m).map(IcedMessage::FileSelectorMessage));
            }
        },
        IcedMessage::MainViewScrolled(o) => {
            context.main_view_scrolled = o;
        },
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(job_result) => match &job_result.kind {
            JobResultKind::FindInChatsError(e) => match &mut context.curr_popup {
                Some(Popup::FindInChats { error, job_id }) if *job_id == job_result.id => {
                    *job_id = None;
                    *error = Some(e.to_string());
                },
                _ => {},
            },
            JobResultKind::FindInChats { regex, matches } => match &context.curr_popup {
                Some(Popup::FindInChats { job_id, .. }) if *job_id == job_result.id => {
                    return Ok(Task::done(IcedMessage::OpenPopup {
                        curr: Popup::FindInChatsResult { regex: regex.to_string(), matches: matches.to_vec() },
                        prev: None,
                    }));
                },
                _ => {},
            },
            JobResultKind::GetFilePreview { .. } => {
                if let Some(c) = &mut context.file_selector_context {
                    return Ok(file_selector::update(c, FileSelectorMessage::BackgroundJobResult(job_result)).map(IcedMessage::FileSelectorMessage));
                }
            },
            _ => {},
        },
        IcedMessage::Notify(_) => unreachable!(),
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
                text!("{title}").color(white()).size(context.zoom * 14.0).into(),
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
                text!("{}", context.now).color(white()).size(context.zoom * 14.0).into(),
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
                    IcedMessage::OpenPopup { curr: Popup::NewProject, prev: None },
                    green(),
                    context.zoom,
                ),
                button(
                    "Config",
                    IcedMessage::OpenPopup { curr: Popup::ProjectConfig, prev: None },
                    blue(),
                    context.zoom,
                ),
                button(
                    "Skills",
                    IcedMessage::OpenPopup { curr: Popup::Skills, prev: None },
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
                    IcedMessage::OpenPopup { curr: Popup::NewChat, prev: None },
                    brown(),
                    context.zoom,
                ),
                button(
                    "Config",
                    IcedMessage::OpenPopup { curr: Popup::ChatConfig, prev: None },
                    blue(),
                    context.zoom,
                ),
                button(
                    "System Prompts",
                    IcedMessage::OpenPopup { curr: Popup::ChatSystemPrompts, prev: None },
                    blue(),
                    context.zoom,
                ),
                button(
                    "Find",
                    IcedMessage::OpenPopup { curr: Popup::FindInChats { error: None, job_id: None }, prev: None },
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
        {
            let (version, commit, build_profile) = build_info();
            Container::new(
                text!("version: {version} ({commit}), build-profile: {build_profile}").color(white()).size(context.zoom * 12.0)
            )
                .padding(context.zoom * 8.0)
                .into()
        },
    ]).spacing(context.zoom * 8.0);
    let c = Container::new(c).style(|_| set_bg(gray(0.16)));

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

    else if let Some(Popup::ProjectConfig | Popup::ExistingProjectConfig(_)) = context.curr_popup {
        let save_action = match context.curr_popup {
            Some(Popup::ExistingProjectConfig(id)) => IcedMessage::SaveProjectConfig(id),
            _ => IcedMessage::SaveGlobalProjectConfig,
        };

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(
                Column::from_vec(vec![
                    config_ui(&context.new_project_config, context.zoom).map(IcedMessage::SetProjectConfig),
                    button("Save", save_action, green(), context.zoom).into(),
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
            ).id(context.popup_scroll_id.clone()).into(), context),
        ]).into();
    }

    else if let Some(Popup::Skills) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_skills_popup(context),
        ]).into();
    }

    else if let Some(Popup::Skill { name }) = &context.curr_popup {
        let skill_editor = Scrollable::new(
            Column::from_vec(vec![
                text!("name").color(white()).size(context.zoom * 18.0).into(),
                TextInput::new("name", &context.short_text_editor_content)
                    .width(context.window_size.width)
                    .size(context.zoom * 14.0)
                    .id(context.short_text_editor_id.clone())
                    .on_input(IcedMessage::EditShortText)
                    .on_submit(IcedMessage::FocusLongTextEdit)
                    .into(),
                text!("description").color(white()).size(context.zoom * 18.0).into(),
                TextEditor::new(&context.long_text_editor_content)
                    .width(context.window_size.width)
                    .size(context.zoom * 14.0)
                    .id(context.long_text_editor_id.clone())
                    .min_height(300)
                    .on_action(IcedMessage::EditLongText)
                    .into(),
                text!("body").color(white()).size(context.zoom * 18.0).into(),
                TextEditor::new(&context.extra_text_editor_content)
                    .width(context.window_size.width)
                    .size(context.zoom * 14.0)
                    .min_height(500)
                    .on_action(IcedMessage::EditExtraText)
                    .into(),
                button(
                    "Save",
                    match name {
                        Some(name) => IcedMessage::UpdateSkill { name: name.to_string() },
                        None => IcedMessage::CreateSkill,
                    },
                    green(),
                    context.zoom,
                ).into(),
            ])
                .padding(context.zoom * 8.0)
                .spacing(context.zoom * 8.0)
        )
            .id(context.popup_scroll_id.clone());

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(skill_editor.into(), context),
        ]).into();
    }

    else if let Some(Popup::NewChat) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_new_chat_popup(context),
        ]).into();
    }

    else if let Some(Popup::ChatConfig | Popup::ExistingChatConfig(_)) = context.curr_popup {
        let save_action = match context.curr_popup {
            Some(Popup::ExistingChatConfig(id)) => IcedMessage::SaveChatConfig(id),
            _ => IcedMessage::SaveGlobalChatConfig,
        };

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(
                Column::from_vec(vec![
                    chat_config_ui1(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig),
                    chat_config_ui2(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig),
                    chat_config_ui3(&context.new_chat_config, &context.system_prompts, context.zoom).map(IcedMessage::SetChatConfig),
                    button("Save", save_action, green(), context.zoom).into(),
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
            ).id(context.popup_scroll_id.clone()).into(), context),
        ]).into();
    }

    else if let Some(Popup::ChatSystemPrompts) = &context.curr_popup {
        let mut column: Vec<Element<IcedMessage>> = context.system_prompts.iter().enumerate().map(
            |(i, system_prompt)| Row::from_vec(vec![
                Container::new(text!("{}", truncate_chars(&system_prompt.replace("\n", "\\n"), 256)).color(white()).size(context.zoom * 14.0))
                    .width(context.zoom * 500.0)
                    .height(context.zoom * 100.0)
                    .padding(context.zoom * 8.0)
                    .style(move |_| set_round_bg(gray(0.2), context.zoom))
                    .into(),
                button("Edit", IcedMessage::OpenPopup {
                    curr: Popup::EditChatSystemPrompt(i),
                    prev: Some(Popup::ChatSystemPrompts),
                }, blue(), context.zoom).into(),
                button("Delete", IcedMessage::OpenPopup {
                    curr: Popup::AskDeleteChatSystemPrompt(i),
                    prev: Some(Popup::ChatSystemPrompts),
                }, red(), context.zoom).into(),
            ])
                .spacing(context.zoom * 8.0)
                .align_y(Vertical::Center)
                .into()
        ).collect();

        column.push(button("Add", IcedMessage::AddChatSystemPrompt, green(), context.zoom).into());
        column.push(Space::new().width(context.window_size.width).height(1.0).into());

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(Column::from_vec(column).spacing(context.zoom * 8.0).align_x(Horizontal::Center)).id(context.popup_scroll_id.clone()).into(), context),
        ]).into();
    }

    else if let Some(Popup::EditChatSystemPrompt(i)) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(Scrollable::new(Column::from_vec(vec![
                TextEditor::new(&context.long_text_editor_content)
                    .size(context.zoom * 14.0)
                    .width(context.window_size.width)
                    .on_action(IcedMessage::EditLongText)
                    .min_height(400)
                    .id(context.long_text_editor_id.clone())
                    .into(),
                button("Save", IcedMessage::EditChatSystemPrompt(i), green(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).align_x(Horizontal::Center)).id(context.popup_scroll_id.clone()).into(), context),
        ]).into();
    }

    else if let Some(Popup::Error(e)) = &context.curr_popup {
        let error = Column::from_vec(vec![
            text!("ERROR").color(red()).size(context.zoom * 21.0).into(),
            text!("{e}").color(white()).size(context.zoom * 14.0).into(),
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
            text!("Delete project `{project_name}`?").color(white()).size(context.zoom * 14.0).into(),
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

    else if let Some(Popup::AskDeleteSkill(name)) = &context.curr_popup {
        let ask = Column::from_vec(vec![
            text!("Delete skill `{name}`?").color(white()).size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::DeleteSkill(name.to_string()), green(), context.zoom).into(),
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
                text!("Delete chat `{title}`?").color(white()).size(context.zoom * 14.0).into()
            } else {
                text!("Delete untitled chat?").color(white()).size(context.zoom * 14.0).into()
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

    else if let Some(Popup::AskDeleteChatSystemPrompt(i)) = context.curr_popup {
        let ask = Column::from_vec(vec![
            text!("Are you sure to delete this system prompt?").color(white()).size(context.zoom * 14.0).into(),
            button("(Y)es", IcedMessage::DeleteChatSystemPrompt(i), green(), context.zoom).into(),
        ])
            .spacing(context.zoom * 20.0)
            .align_x(Horizontal::Center)
            .width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(ask.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::FindInChats { job_id, error }) = &context.curr_popup {
        let mut text_editor = TextInput::new("regex", &context.short_text_editor_content)
            .size(context.zoom * 14.0)
            .id(context.short_text_editor_id.clone());

        if job_id.is_none() {
            text_editor = text_editor.on_input(IcedMessage::EditShortText).on_submit(IcedMessage::FindInChats);
        }

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            into_popup(
                Column::from_vec(vec![
                    text_editor.into(),
                    if job_id.is_some() {
                        text!("Finding...").color(white()).size(context.zoom * 14.0).into()
                    } else {
                        Space::new().into()
                    },
                    if let Some(error) = error {
                        text!("{error}").color(white()).size(context.zoom * 14.0).color(red()).into()
                    } else {
                        Space::new().into()
                    },
                    if job_id.is_some() {
                        disabled_button("Find", gray(0.4), context.zoom).padding(context.zoom * 20.0).into()
                    } else {
                        button("Find", IcedMessage::FindInChats, green(), context.zoom).padding(context.zoom * 20.0).into()
                    },
                ])
                    .spacing(context.zoom * 20.0)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill)
                    .into(),
                context,
            ).into(),
        ]).into();
    }

    else if let Some(Popup::FindInChatsResult { regex, matches }) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_find_in_chats_result(regex, matches, context),
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
        button("(M)odel Store", IcedMessage::OpenPopup {
            curr: Popup::ModelStore,
            prev: None,
        }, blue(), context.zoom),
        button("(H)elp", IcedMessage::OpenPopup {
            curr: Popup::Help,
            prev: None,
        }, pink(), context.zoom),
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
                format!(" ({})", prettify_timestamp(project.updated_at))
            };

            // TODO: see summaries of the session
            let mut buttons = vec![
                if context.working_dir_tab_indexes.contains_key(&project.neukgu_id) {
                    button("Switch", IcedMessage::NewTab { tab: Tab::WorkingDir(project.working_dir.to_string()), force_new_tab: false }, green(), context.zoom)
                } else {
                    button("Launch", IcedMessage::NewTab { tab: Tab::WorkingDir(project.working_dir.to_string()), force_new_tab: false }, green(), context.zoom)
                },
                button("Browse", IcedMessage::NewTab { tab: Tab::Browser { dir: project.working_dir.to_string(), file: None }, force_new_tab: false }, skyblue(), context.zoom),
                button("Config", IcedMessage::OpenPopup {
                    curr: Popup::ExistingProjectConfig(project.neukgu_id),
                    prev: None,
                }, blue(), context.zoom),
                button("Instruction", IcedMessage::OpenPopup {
                    curr: Popup::Instruction { working_dir: project.working_dir.to_string() },
                    prev: None,
                }, yellow(), context.zoom),
            ];

            if project.is_in_global_index_dir {
                buttons.push(button(
                    "Delete",
                    IcedMessage::OpenPopup {
                        curr: Popup::AskDeleteProject {
                            project_name: path.to_string(),
                            working_dir: project.working_dir.to_string(),
                        },
                        prev: None,
                    },
                    red(),
                    context.zoom,
                ));
            }

            let buttons = if context.curr_popup.is_some() {
                buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
            } else {
                buttons.into_iter().map(|button| button.into()).collect()
            };

            Container::new(
                Column::from_vec(vec![
                    text!("{path}{elapsed}").color(white()).size(context.zoom * 14.0).width(context.window_size.width).into(),
                    if let Some(error) = &project.error {
                        text!("{error}").color(red()).size(context.zoom * 14.0).into()
                    } else {
                        Row::from_vec(buttons).spacing(context.zoom * 4.0).into()
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
        |chat| {
            let buttons = vec![
                button("Open", IcedMessage::NewTab { tab: Tab::Chat(chat.id), force_new_tab: false }, green(), context.zoom),
                button("Config", IcedMessage::OpenPopup {
                    curr: Popup::ExistingChatConfig(chat.id),
                    prev: None,
                }, blue(), context.zoom),
                button("Delete", IcedMessage::OpenPopup {
                    curr: Popup::AskDeleteChat { id: chat.id, title: chat.title.clone() },
                    prev: None,
                }, red(), context.zoom),
            ];

            let buttons = if context.curr_popup.is_some() {
                buttons.into_iter().map(|button| button.on_press_maybe(None).into()).collect()
            } else {
                buttons.into_iter().map(|button| button.into()).collect()
            };

            Container::new(
                Column::from_vec(vec![
                    text!(
                        "{} ({})",
                        chat.title.as_ref().unwrap_or(&String::from("untitled")),
                        prettify_timestamp(chat.updated_at),
                    )
                        .color(white())
                        .size(context.zoom * 14.0)
                        .width(context.window_size.width)
                        .into(),
                    Row::from_vec(buttons).spacing(context.zoom * 8.0).into(),
                ]).spacing(context.zoom * 8.0)
            )
                .style(|_| set_round_bg(gray(0.3), context.zoom))
                .padding(context.zoom * 8.0)
                .into()
        }
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
            text!("1. ").color(white()).size(context.zoom * 14.0).into(),
            circle(context.zoom * 6.0, white()),
            text!("Index").color(white()).size(context.zoom * 14.0).into(),
        ])
            .spacing(context.zoom * 4.0)
            .align_y(Vertical::Center),
        false,
        context.window_size.width,
        context.zoom,
    ).into()];

    column.extend(context.current_tabs.iter().enumerate().map(
        |(i, tab)| {
            let mut texts: Vec<Element<IcedMessage>> = vec![text!("{}", tab.title).color(white()).size(context.zoom * 14.0).into()];

            if let Some(status) = &tab.status {
                texts.push(text!("{status}").color(white()).size(context.zoom * 14.0).into());
            }

            if let Some(error) = &tab.error {
                texts.push(text!("{error}").color(red()).size(context.zoom * 14.0).into());
            }

            let tab_view = draw_bg(
                Row::from_vec(vec![
                    text!("{}. ", i + 2).color(white()).size(context.zoom * 14.0).into(),
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
        .on_input(IcedMessage::EditShortText)
        .on_submit(IcedMessage::FocusLongTextEdit);

    let long_text_editor = TextEditor::new(&context.long_text_editor_content)
        .placeholder("What do you want neukgu to do?")
        .size(context.zoom * 14.0)
        .id(context.long_text_editor_id.clone())
        .min_height(400)
        .on_action(IcedMessage::EditLongText);

    let file_selector = match &context.file_selector_context {
        Some(c) => file_selector::view(c, true, context.window_size.width * 0.65, context.zoom * 480.0, context.zoom).map(IcedMessage::FileSelectorMessage).into(),
        None => button("Attach", IcedMessage::OpenFileSelector, skyblue(), context.zoom).into(),
    };

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                long_text_editor.into(),
                file_selector,
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

fn render_skills_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut column: Vec<Element<IcedMessage>> = context.skills.iter().map(
        |(name, maybe_skill)| Container::new(
            Row::from_vec(vec![
                Column::from_vec(vec![
                    text!("{name}").color(white()).size(context.zoom * 18.0).into(),
                    Container::new(match maybe_skill {
                        Ok(skill) => text!("{}", skill.description).color(white()).size(context.zoom * 14.0),
                        Err(e) => text!("ERROR: {e}").color(red()).size(context.zoom * 14.0),
                    })
                        .padding(context.zoom * 8.0)
                        .width(context.zoom * 500.0)
                        .style(|_| set_bg(black()))
                        .into(),
                ])
                    .spacing(context.zoom * 8.0)
                    .into(),
                Column::from_vec(vec![
                    button("Edit", IcedMessage::OpenPopup { curr: Popup::Skill { name: Some(name.to_string()) }, prev: Some(Popup::Skills) }, blue(), context.zoom).into(),
                    button("Delete", IcedMessage::OpenPopup { curr: Popup::AskDeleteSkill(name.to_string()), prev: Some(Popup::Skills) }, red(), context.zoom).into(),
                ])
                    .spacing(context.zoom * 8.0)
                    .width(context.zoom * 100.0)
                    .align_x(Horizontal::Center)
                    .into(),
            ])
                .align_y(Vertical::Center)
                .spacing(context.zoom * 8.0)
        )
            .padding(context.zoom * 8.0)
            .style(|_| set_round_bg(gray(0.2), context.zoom))
            .into()
    ).collect();

    column.push(
        button("Add", IcedMessage::OpenPopup { curr: Popup::Skill { name: None }, prev: Some(Popup::Skills) }, green(), context.zoom).into(),
    );
    column.push(Space::new().width(context.window_size.width).into());

    into_popup(
        Scrollable::new(
            Column::from_vec(column)
                .align_x(Horizontal::Center)
                .padding(context.zoom * 8.0)
                .spacing(context.zoom * 8.0)
        )
            .id(context.popup_scroll_id.clone())
            .into(),
        context,
    )
}

fn render_new_chat_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let short_text_editor = TextInput::new("(untitled)", &context.short_text_editor_content)
        .size(context.zoom * 14.0)
        .id(context.short_text_editor_id.clone())
        .on_input(IcedMessage::EditShortText);

    into_popup(
        Scrollable::new(
            Column::from_vec(vec![
                short_text_editor.into(),
                chat_config_ui1(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig).into(),
                chat_config_ui2(&context.new_chat_config, context.zoom).map(IcedMessage::SetChatConfig).into(),
                chat_config_ui3(&context.new_chat_config, &context.system_prompts, context.zoom).map(IcedMessage::SetChatConfig).into(),
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
        let mut cell = Container::new(text!(" ").size(zoom * 12.0));
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

fn render_find_in_chats_result<'r, 'm, 'c>(regex: &'r str, matches: &'m [(ChatId, Vec<MatchPreview>)], context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut column: Vec<Element<IcedMessage>> = vec![
        text!("Find: {regex}").color(white()).size(context.zoom * 14.0).into(),
    ];
    let chat_titles: HashMap<ChatId, String> = context.recent_chats.iter().filter_map(
        |chat| match &chat.title {
            Some(title) => Some((chat.id, title.to_string())),
            None => None,
        }
    ).collect();

    column.extend(matches.iter().map(
        |(chat_id, previews)| {
            let mut column = vec![
                Row::from_vec(vec![
                    text!("{}", chat_titles.get(chat_id).cloned().unwrap_or(String::from("untitled chat"))).color(white()).size(context.zoom * 18.0).into(),
                    button("Open", IcedMessage::NewTab { tab: Tab::Chat(*chat_id), force_new_tab: false }, green(), context.zoom).into(),
                ]).spacing(context.zoom * 8.0).align_y(Vertical::Center).into(),
            ];

            for preview in previews.iter() {
                let row: Vec<Element<IcedMessage>> = vec![
                    text!("{}{}", if preview.pre_truncated { "..." } else { "" }, preview.pre).color(white()).size(context.zoom * 14.0).into(),
                    Container::new(text!("{}", preview.matched).color(black()).size(context.zoom * 14.0)).style(|_| set_bg(white())).into(),
                    text!("{}{}", preview.post, if preview.post_truncated { "..." } else { "" }).color(white()).size(context.zoom * 14.0).into(),
                ];

                column.push(Row::from_vec(row).into());
            }

            Container::new(
                Column::from_vec(column)
                    .width(context.window_size.width)
                    .padding(context.zoom * 8.0)
                    .spacing(context.zoom * 4.0)
            ).style(|_| set_round_bg(gray(0.2), context.zoom)).into()
        }
    ));

    if matches.is_empty() {
        column.push(text!("No matches found").color(white()).size(context.zoom * 14.0).into());
    }

    into_popup(
        Scrollable::new(
            Column::from_vec(column)
                .spacing(context.zoom * 8.0),
        )
            .id(context.popup_scroll_id.clone())
            .into(),
        context,
    )
}
