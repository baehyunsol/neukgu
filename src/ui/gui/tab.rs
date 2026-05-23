use super::{blue, brown, green, red, skyblue, yellow};
use super::browser::{
    self,
    IcedContext as BrowserContext,
    IcedMessage as BrowserMessage,
};
use super::chat::{
    self,
    IcedContext as ChatContext,
    IcedMessage as ChatMessage,
};
use super::error::{
    self,
    IcedContext as ErrorContext,
    IcedMessage as ErrorMessage,
};
use super::scratch_pad::Content as ScratchPadContent;
use super::tabs::Tab;
use super::worker::{Job, JobResult};
use super::working_dir::{
    self,
    IcedContext as WorkingDirContext,
    IcedMessage as WorkingDirMessage,
};
use crate::{NeukguId, prettify_timestamp, truncate_chars};
use iced::{Color, Element, Size, Task};
use iced::keyboard::{Key, Modifiers};
use iced::widget::Id;
use ragit_fs::{basename, exists, join};
use std::collections::HashMap;

pub struct IcedContext {
    pub id: TabId,
    pub cwd: String,
    pub api_keys: HashMap<String, String>,
    pub local: LocalContext,
}

impl IcedContext {
    pub fn new(
        home_dir: &str,
        api_keys: HashMap<String, String>,
        tab: Tab,
        window_size: Size,
    ) -> IcedContext {
        match tab {
            Tab::Browser { dir, file } => {
                let local_context = match BrowserContext::new(home_dir, &dir, &file, window_size) {
                    Ok(c) => LocalContext::Browser(c),
                    Err(e) => LocalContext::Error(ErrorContext::new(format!("{e:?}"), window_size, 1.0)),
                };

                IcedContext {
                    id: TabId(rand::random::<u64>()),
                    cwd: dir,
                    api_keys,
                    local: local_context,
                }
            },
            Tab::Chat(id) => {
                let local_context = match ChatContext::new(id, api_keys.clone(), window_size) {
                    Ok(c) => LocalContext::Chat(c),
                    Err(e) => LocalContext::Error(ErrorContext::new(format!("{e:?}"), window_size, 1.0)),
                };

                IcedContext {
                    id: TabId(rand::random::<u64>()),
                    cwd: home_dir.to_string(),
                    api_keys,
                    local: local_context,
                }
            },
            Tab::WorkingDir(dir) => {
                // TODO: make `no_backend` configurable
                let local_context = match WorkingDirContext::new(false, api_keys.clone(), &dir, window_size, 1.0) {
                    Ok(c) => LocalContext::WorkingDir(c),
                    Err(e) => LocalContext::Error(ErrorContext::new(format!("{e:?}"), window_size, 1.0)),
                };

                IcedContext {
                    id: TabId(rand::random::<u64>()),
                    cwd: dir,
                    api_keys,
                    local: local_context,
                }
            },
        }
    }

    pub fn get_title_and_flag(&self, full_path: bool) -> (String, Color) {
        match &self.local {
            LocalContext::Browser(c) => {
                let title = match &c.curr_popup {
                    Some(browser::Popup::PreviewFile { path } | browser::Popup::PreviewSymlink { path }) => format!("Reading {}", if full_path { path.to_string() } else { basename(path).unwrap() }),
                    _ => format!("Browse {}/", if full_path { c.cwd.to_string() } else { basename(&c.cwd).unwrap() }),
                };
                (title, skyblue())
            },
            LocalContext::Chat(c) => (
                format!("Chat {}", c.chat.title.as_ref().unwrap_or(&String::new())),
                brown(),
            ),
            LocalContext::WorkingDir(c) => {
                let flag = match 0 {
                    _ if c.fe_context.curr_error().is_some() => red(),
                    _ if c.llm_request.is_some() => blue(),
                    _ if c.is_paused => yellow(),
                    _ => green(),
                };

                (format!("Working Dir {}/", if full_path { c.fe_context.working_dir.to_string() } else { basename(&c.fe_context.working_dir).unwrap() }), flag)
            },
            LocalContext::Error(c) => (c.message.to_string(), red()),
        }
    }

    pub fn get_scroll_id(&self) -> Option<Id> {
        match &self.local {
            LocalContext::Browser(c) => Some(c.entry_view_id.clone()),
            LocalContext::Chat(c) => Some(c.chat_view_id.clone()),
            LocalContext::WorkingDir(c) => Some(c.turn_view_id.clone()),
            LocalContext::Error(_) => None,
        }
    }

    pub fn get_preview(&self, index: usize) -> TabPreview {
        let mut neukgu_id = None;
        let (title, flag) = self.get_title_and_flag(true);
        let (status, error) = match &self.local {
            LocalContext::Browser(c) => match &c.curr_popup {
                Some(browser::Popup::RunResult { command, started_at, output: None, error, .. }) => (
                    Some(format!(
                        "Running `{}`... ({})",
                        truncate_chars(command, 42),
                        prettify_timestamp(*started_at),
                    )),
                    error.clone(),
                ),
                _ => (None, None),
            },
            LocalContext::Chat(c) => {
                let status = if c.bg_job.is_some() {
                    Some(String::from("Processing..."))
                } else {
                    None
                };
                (status, c.bg_error.clone())
            },
            LocalContext::WorkingDir(c) => {
                neukgu_id = Some(c.fe_context.neukgu_id);

                // If the dir doesn't exist, `c.fe_context.curr_status()` will take long to finish, and it's really bad for
                // the view function to take long time.
                if !exists(&join(&c.fe_context.working_dir, ".neukgu").unwrap_or(c.fe_context.working_dir.to_string())) {
                    (Some(String::from("???")), Some(String::from("Cannot read the index dir")))
                }

                else {
                    (
                        Some(c.fe_context.curr_status()),
                        c.fe_context.curr_error(),
                    )
                }
            },
            LocalContext::Error(_) => (None, None),
        };

        TabPreview {
            id: self.id,
            neukgu_id,
            index,
            flag,
            title,
            status,
            error,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Browser(BrowserMessage),
    Chat(ChatMessage),
    WorkingDir(WorkingDirMessage),
    Error(ErrorMessage),
    Tick { frame: usize, force_update: bool },
    KeyPressed { key: Key, modifiers: Modifiers },
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    WindowResized(Size),
    Focus,
    OpenScratchPad { title: Option<String>, content: ScratchPadContent },

    // Kill: The caller wants to kill this tab.
    // Dead: Tell the caller that this tab is okay to be closed.
    Kill,
    Dead,
}

#[derive(Debug)]
pub enum LocalContext {
    Browser(BrowserContext),
    Chat(ChatContext),
    WorkingDir(WorkingDirContext),
    Error(ErrorContext),
}

impl LocalContext {
    pub fn window_size(&self) -> Size {
        match self {
            LocalContext::Browser(c) => c.window_size,
            LocalContext::Chat(c) => c.window_size,
            LocalContext::WorkingDir(c) => c.window_size,
            LocalContext::Error(c) => c.window_size,
        }
    }

    pub fn zoom(&self) -> f32 {
        match self {
            LocalContext::Browser(c) => c.zoom,
            LocalContext::Chat(c) => c.zoom,
            LocalContext::WorkingDir(c) => c.zoom,
            LocalContext::Error(c) => c.zoom,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TabId(u64);

pub struct TabPreview {
    pub id: TabId,
    pub neukgu_id: Option<NeukguId>,  // if it's `Tab::WorkingDir`
    pub index: usize,
    pub flag: Color,
    pub title: String,
    pub status: Option<String>,
    pub error: Option<String>,
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match (&mut context.local, message) {
        (context, IcedMessage::Browser(BrowserMessage::Error(e)) | IcedMessage::Chat(ChatMessage::Error(e)) | IcedMessage::WorkingDir(WorkingDirMessage::Error(e))) => {
            *context = LocalContext::Error(ErrorContext::new(e, context.window_size(), context.zoom()));
            Task::none()
        },
        (
            _,
            IcedMessage::Browser(BrowserMessage::OpenScratchPad { title, content }) |
            IcedMessage::Chat(ChatMessage::OpenScratchPad { title, content }) |
            IcedMessage::WorkingDir(WorkingDirMessage::OpenScratchPad { title, content }),
        ) => Task::done(IcedMessage::OpenScratchPad { title, content }),
        (_, IcedMessage::Browser(BrowserMessage::Dead) | IcedMessage::Chat(ChatMessage::Dead) | IcedMessage::WorkingDir(WorkingDirMessage::Dead) | IcedMessage::Error(ErrorMessage::Dead)) => {
            Task::done(IcedMessage::Dead)
        },
        (_, IcedMessage::Browser(BrowserMessage::BackgroundJob(job)) | IcedMessage::Chat(ChatMessage::BackgroundJob(job)) | IcedMessage::WorkingDir(WorkingDirMessage::BackgroundJob(job))) => {
            Task::done(IcedMessage::BackgroundJob(job))
        },
        (local_context, IcedMessage::Browser(BrowserMessage::Launch { path })) => {
            // TODO: make `no_backend` configurable
            match WorkingDirContext::new(false, context.api_keys.clone(), &path, local_context.window_size(), local_context.zoom()) {
                Ok(c) => {
                    *local_context = LocalContext::WorkingDir(c);
                },
                Err(e) => {
                    *local_context = LocalContext::Error(ErrorContext::new(format!("{e:?}"), local_context.window_size(), local_context.zoom()));
                },
            }

            Task::none()
        },
        (LocalContext::Browser(c), IcedMessage::Tick { frame, force_update }) => browser::update(c, BrowserMessage::Tick { frame, force_update }).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::KeyPressed { key, modifiers }) => browser::update(c, BrowserMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::Browser(c), IcedMessage::Browser(BrowserMessage::ChDir(path))) => {
            context.cwd = path.to_string();
            browser::update(c, BrowserMessage::ChDir(path)).map(|t| IcedMessage::Browser(t))
        },
        (LocalContext::Browser(c), IcedMessage::BackgroundJobResult(r)) => browser::update(c, BrowserMessage::BackgroundJobResult(r)).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::Focus) => browser::update(c, BrowserMessage::Focus).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::Browser(m)) => browser::update(c, m).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Chat(c), IcedMessage::Tick { frame, force_update }) => chat::update(c, ChatMessage::Tick { frame, force_update }).map(|t| IcedMessage::Chat(t)),
        (LocalContext::Chat(c), IcedMessage::KeyPressed { key, modifiers }) => chat::update(c, ChatMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Chat(t)),
        (LocalContext::Chat(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::Chat(c), IcedMessage::BackgroundJobResult(r)) => chat::update(c, ChatMessage::BackgroundJobResult(r)).map(|t| IcedMessage::Chat(t)),
        (LocalContext::Chat(c), IcedMessage::Focus) => chat::update(c, ChatMessage::Focus).map(|t| IcedMessage::Chat(t)),
        (LocalContext::Chat(c), IcedMessage::Chat(m)) => chat::update(c, m).map(|t| IcedMessage::Chat(t)),
        (LocalContext::WorkingDir(c), IcedMessage::Tick { frame, force_update }) => working_dir::update(c, WorkingDirMessage::Tick { frame, force_update }).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::KeyPressed { key, modifiers }) => working_dir::update(c, WorkingDirMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::WorkingDir(c), IcedMessage::BackgroundJobResult(r)) => working_dir::update(c, WorkingDirMessage::BackgroundJobResult(r)).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::Focus) => working_dir::update(c, WorkingDirMessage::Focus).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::WorkingDir(m)) => working_dir::update(c, m).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::Error(c), IcedMessage::KeyPressed { key, modifiers }) => error::update(c, ErrorMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Error(t)),
        (LocalContext::Error(_), IcedMessage::Tick { .. } | IcedMessage::Focus) => Task::none(),
        (context, IcedMessage::Kill) => match context {
            LocalContext::Browser(c) => browser::update(c, BrowserMessage::Kill).map(|m| IcedMessage::Browser(m)),
            LocalContext::Chat(c) => chat::update(c, ChatMessage::Kill).map(|m| IcedMessage::Chat(m)),
            LocalContext::WorkingDir(c) => working_dir::update(c, WorkingDirMessage::Kill).map(|m| IcedMessage::WorkingDir(m)),
            LocalContext::Error(c) => error::update(c, ErrorMessage::Kill).map(|m| IcedMessage::Error(m)),
        },
        (context, message) => panic!("{context:?}\n{message:?}"),
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    match &context.local {
        LocalContext::Browser(c) => browser::view(c).map(|m| IcedMessage::Browser(m)),
        LocalContext::Chat(c) => chat::view(c).map(|m| IcedMessage::Chat(m)),
        LocalContext::WorkingDir(c) => working_dir::view(c).map(|m| IcedMessage::WorkingDir(m)),
        LocalContext::Error(e) => error::view(e).map(|m| IcedMessage::Error(m)),
    }
}
