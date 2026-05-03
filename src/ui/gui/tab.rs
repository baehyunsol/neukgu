use super::{blue, green, red, skyblue, yellow};
use super::browser::{
    self,
    IcedContext as BrowserContext,
    IcedMessage as BrowserMessage,
};
use super::error::{
    self,
    IcedContext as ErrorContext,
    IcedMessage as ErrorMessage,
};
use super::tabs::Tab;
use super::working_dir::{
    self,
    IcedContext as WorkingDirContext,
    IcedMessage as WorkingDirMessage,
};
use iced::{Color, Element, Size, Task};
use iced::keyboard::{Key, Modifiers};
use iced::widget::Id;
use ragit_fs::{basename, parent};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TabId(u64);

pub struct TabPreview {
    pub id: TabId,
    pub index: usize,
    pub flag: Color,
    pub title: String,
    pub status: Option<String>,
    pub error: Option<String>,
}

pub struct IcedContext {
    pub id: TabId,
    pub home_dir: String,
    pub cwd: String,
    pub local: LocalContext,
}

impl IcedContext {
    pub fn get_title_and_flag(&self, full_path: bool) -> (String, Color) {
        match &self.local {
            LocalContext::Browser(c) => {
                let title = match &c.curr_popup {
                    Some(browser::Popup::Preview { path }) => format!("Reading {}", if full_path { path.to_string() } else { basename(path).unwrap() }),
                    _ => format!("Browse {}/", if full_path { c.cwd.to_string() } else { basename(&c.cwd).unwrap() }),
                };
                (title, skyblue())
            },
            LocalContext::WorkingDir(c) => {
                let flag = match 0 {
                    _ if c.fe_context.curr_error().is_some() => red(),
                    _ if c.llm_request.is_some() => yellow(),
                    _ if c.is_paused => blue(),
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
            LocalContext::WorkingDir(c) => Some(c.turn_view_id.clone()),
            LocalContext::Error(_) => None,
        }
    }

    pub fn get_preview(&self, index: usize) -> TabPreview {
        let (title, flag) = self.get_title_and_flag(true);
        let (status, error) = match &self.local {
            LocalContext::WorkingDir(c) => (
                Some(c.fe_context.curr_status()),
                c.fe_context.curr_error(),
            ),
            LocalContext::Browser(_) | LocalContext::Error(_) => (None, None),
        };

        TabPreview {
            id: self.id,
            index,
            flag,
            title,
            status,
            error,
        }
    }
}

pub enum LocalContext {
    Browser(BrowserContext),
    WorkingDir(WorkingDirContext),
    Error(ErrorContext),
}

impl LocalContext {
    pub fn window_size(&self) -> Size {
        match self {
            LocalContext::Browser(c) => c.window_size,
            LocalContext::WorkingDir(c) => c.window_size,
            LocalContext::Error(c) => c.window_size,
        }
    }

    pub fn zoom(&self) -> f32 {
        match self {
            LocalContext::Browser(c) => c.zoom,
            LocalContext::WorkingDir(c) => c.zoom,
            LocalContext::Error(c) => c.zoom,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Browser(BrowserMessage),
    WorkingDir(WorkingDirMessage),
    Error(ErrorMessage),
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
}

pub fn boot(home_dir: &str, tab: Tab, window_size: Size) -> IcedContext {
    match tab {
        Tab::Browser { dir, file } => {
            let local_context = match browser::try_boot(window_size, home_dir, &dir, &file) {
                Ok(c) => LocalContext::Browser(c),
                Err(e) => LocalContext::Error(error::boot(format!("{e:?}"), window_size, 1.0)),
            };

            IcedContext {
                id: TabId(rand::random::<u64>()),
                home_dir: home_dir.to_string(),
                cwd: dir,
                local: local_context,
            }
        },
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match (&mut context.local, message) {
        (context, IcedMessage::Browser(BrowserMessage::Error(e)) | IcedMessage::WorkingDir(WorkingDirMessage::Error(e))) => {
            *context = LocalContext::Error(error::boot(e, context.window_size(), context.zoom()));
            Task::none()
        },
        (context, IcedMessage::Browser(BrowserMessage::Launch { path })) => {
            // TODO: make `no_backend` configurable
            match working_dir::try_boot(false, &path, context.window_size(), context.zoom()) {
                Ok(c) => {
                    *context = LocalContext::WorkingDir(c);
                },
                Err(e) => {
                    *context = LocalContext::Error(error::boot(format!("{e:?}"), context.window_size(), context.zoom()));
                },
            }

            Task::none()
        },
        (LocalContext::Browser(c), IcedMessage::Tick) => browser::update(c, BrowserMessage::Tick).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::KeyPressed { key, modifiers }) => browser::update(c, BrowserMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Browser(t)),
        (LocalContext::Browser(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::Browser(c), IcedMessage::Browser(BrowserMessage::ChDir(path))) => {
            context.cwd = path.to_string();
            browser::update(c, BrowserMessage::ChDir(path)).map(|t| IcedMessage::Browser(t))
        },
        (LocalContext::Browser(c), IcedMessage::Browser(m)) => browser::update(c, m).map(|t| IcedMessage::Browser(t)),
        (LocalContext::WorkingDir(c), IcedMessage::Tick) => working_dir::update(c, WorkingDirMessage::Tick).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::KeyPressed { key, modifiers }) => working_dir::update(c, WorkingDirMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::WorkingDir(c), IcedMessage::WorkingDir(m)) => working_dir::update(c, m).map(|t| IcedMessage::WorkingDir(t)),
        (c, IcedMessage::Error(ErrorMessage::Okay)) => {
            match browser::try_boot(c.window_size(), &context.home_dir, &context.cwd, &None) {
                Ok(l) => {
                    *c = LocalContext::Browser(l);
                },
                Err(e) => match parent(&context.cwd) {
                    Ok(parent) => match browser::try_boot(c.window_size(), &context.home_dir, &parent, &None) {
                        Ok(l) => {
                            context.cwd = parent.to_string();
                            *c = LocalContext::Browser(l);
                        },
                        Err(_) => match std::env::var("HOME") {
                            Ok(home) => match browser::try_boot(c.window_size(), &home, &home, &None) {
                                Ok(l) => {
                                    context.cwd = home.to_string();
                                    *c = LocalContext::Browser(l);
                                },
                                Err(_) => {
                                    *c = LocalContext::Error(error::boot(format!("{e:?}"), c.window_size(), c.zoom()));
                                },
                            },
                            Err(_) => {
                                *c = LocalContext::Error(error::boot(format!("{e:?}"), c.window_size(), c.zoom()));
                            },
                        },
                    },
                    Err(_) => match std::env::var("HOME") {
                        Ok(home) => match browser::try_boot(c.window_size(), &home, &home, &None) {
                            Ok(l) => {
                                context.cwd = home.to_string();
                                *c = LocalContext::Browser(l);
                            },
                            Err(_) => {
                                *c = LocalContext::Error(error::boot(format!("{e:?}"), c.window_size(), c.zoom()));
                            },
                        },
                        Err(_) => {
                            *c = LocalContext::Error(error::boot(format!("{e:?}"), c.window_size(), c.zoom()));
                        },
                    },
                },
            }

            Task::none()
        },
        (LocalContext::Error(c), IcedMessage::KeyPressed { key, modifiers }) => error::update(c, ErrorMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Error(t)),
        (LocalContext::Error(_), IcedMessage::Tick) => Task::none(),
        _ => unreachable!(),
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    match &context.local {
        LocalContext::Browser(c) => browser::view(c).map(|m| IcedMessage::Browser(m)),
        LocalContext::WorkingDir(c) => working_dir::view(c).map(|m| IcedMessage::WorkingDir(m)),
        LocalContext::Error(e) => error::view(e).map(|m| IcedMessage::Error(m)),
    }
}
