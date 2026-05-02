use super::{blue, green, red, yellow, white};
use super::error::{
    self,
    IcedContext as ErrorContext,
    IcedMessage as ErrorMessage,
};
use super::launcher::{
    self,
    IcedContext as LauncherContext,
    IcedMessage as LauncherMessage,
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

pub struct IcedContext {
    pub global: GlobalContext,
    pub local: LocalContext,
}

impl IcedContext {
    pub fn get_title_and_flag(&self) -> (String, Color) {
        match &self.local {
            LocalContext::Launcher(c) => {
                let title = match &c.curr_popup {
                    Some(launcher::Popup::Preview { path }) => format!("Reading {}", basename(path).unwrap()),
                    _ => format!("Browse {}/", basename(&c.cwd).unwrap()),
                };
                (title, white())
            },
            LocalContext::WorkingDir(c) => {
                let flag = match 0 {
                    _ if c.fe_context.curr_error().is_some() => red(),
                    _ if c.llm_request.is_some() => yellow(),
                    _ if c.is_paused => blue(),
                    _ => green(),
                };

                (format!("Working Dir {}/", basename(&c.fe_context.working_dir).unwrap()), flag)
            },
            LocalContext::Error(c) => (c.message.to_string(), red()),
        }
    }

    pub fn get_scroll_id(&self) -> Option<Id> {
        match &self.local {
            LocalContext::Launcher(c) => Some(c.entry_view_id.clone()),
            LocalContext::WorkingDir(c) => Some(c.turn_view_id.clone()),
            LocalContext::Error(_) => None,
        }
    }
}

pub struct GlobalContext {
    pub cwd: String,
}

pub enum LocalContext {
    Launcher(LauncherContext),
    WorkingDir(WorkingDirContext),
    Error(ErrorContext),
}

impl LocalContext {
    pub fn window_size(&self) -> Size {
        match self {
            LocalContext::Launcher(c) => c.window_size,
            LocalContext::WorkingDir(c) => c.window_size,
            LocalContext::Error(c) => c.window_size,
        }
    }

    pub fn zoom(&self) -> f32 {
        match self {
            LocalContext::Launcher(c) => c.zoom,
            LocalContext::WorkingDir(c) => c.zoom,
            LocalContext::Error(c) => c.zoom,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Launcher(LauncherMessage),
    WorkingDir(WorkingDirMessage),
    Error(ErrorMessage),
    Tick,
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
}

pub fn boot(tab: Tab, window_size: Size) -> IcedContext {
    match tab {
        Tab::Browser { dir, file } => {
            let local_context = match launcher::try_boot(window_size, &dir, &file) {
                Ok(c) => LocalContext::Launcher(c),
                Err(e) => LocalContext::Error(error::boot(format!("{e:?}"), window_size, 1.0)),
            };

            IcedContext {
                global: GlobalContext { cwd: dir },
                local: local_context,
            }
        },
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match (&mut context.local, message) {
        (context, IcedMessage::Launcher(LauncherMessage::Error(e)) | IcedMessage::WorkingDir(WorkingDirMessage::Error(e))) => {
            *context = LocalContext::Error(error::boot(e, context.window_size(), context.zoom()));
            Task::none()
        },
        (context, IcedMessage::Launcher(LauncherMessage::Launch { path })) => {
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
        (LocalContext::Launcher(c), IcedMessage::Tick) => launcher::update(c, LauncherMessage::Tick).map(|t| IcedMessage::Launcher(t)),
        (LocalContext::Launcher(c), IcedMessage::KeyPressed { key, modifiers }) => launcher::update(c, LauncherMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::Launcher(t)),
        (LocalContext::Launcher(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::Launcher(c), IcedMessage::Launcher(LauncherMessage::ChDir(path))) => {
            context.global.cwd = path.to_string();
            launcher::update(c, LauncherMessage::ChDir(path)).map(|t| IcedMessage::Launcher(t))
        },
        (LocalContext::Launcher(c), IcedMessage::Launcher(m)) => launcher::update(c, m).map(|t| IcedMessage::Launcher(t)),
        (LocalContext::WorkingDir(c), IcedMessage::Tick) => working_dir::update(c, WorkingDirMessage::Tick).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::KeyPressed { key, modifiers }) => working_dir::update(c, WorkingDirMessage::KeyPressed { key, modifiers }).map(|t| IcedMessage::WorkingDir(t)),
        (LocalContext::WorkingDir(c), IcedMessage::WindowResized(s)) => {
            c.window_size = s;
            Task::none()
        },
        (LocalContext::WorkingDir(c), IcedMessage::WorkingDir(m)) => working_dir::update(c, m).map(|t| IcedMessage::WorkingDir(t)),
        (c, IcedMessage::Error(ErrorMessage::Okay)) => {
            match launcher::try_boot(c.window_size(), &context.global.cwd, &None) {
                Ok(l) => {
                    *c = LocalContext::Launcher(l);
                },
                Err(e) => match parent(&context.global.cwd) {
                    Ok(parent) => match launcher::try_boot(c.window_size(), &parent, &None) {
                        Ok(l) => {
                            context.global.cwd = parent.to_string();
                            *c = LocalContext::Launcher(l);
                        },
                        Err(_) => match std::env::var("HOME") {
                            Ok(home) => match launcher::try_boot(c.window_size(), &home, &None) {
                                Ok(l) => {
                                    context.global.cwd = home.to_string();
                                    *c = LocalContext::Launcher(l);
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
                        Ok(home) => match launcher::try_boot(c.window_size(), &home, &None) {
                            Ok(l) => {
                                context.global.cwd = home.to_string();
                                *c = LocalContext::Launcher(l);
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
        LocalContext::Launcher(c) => launcher::view(c).map(|m| IcedMessage::Launcher(m)),
        LocalContext::WorkingDir(c) => working_dir::view(c).map(|m| IcedMessage::WorkingDir(m)),
        LocalContext::Error(e) => error::view(e).map(|m| IcedMessage::Error(m)),
    }
}
