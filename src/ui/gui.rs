use super::{FeContext, Truncation, spawn_be_process};
use crate::Error;
use iced::{Background, Color, Element, Event, Font, Length, Size, Subscription, Task, Theme};
use iced::border::{Border, Radius};
use iced::keyboard::{Event as KeyboardEvent, Key, Modifiers};
use iced::time::{self, Duration};
use iced::widget::{Container, Space, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::Style;
use iced::window::Event as WindowEvent;
use ragit_fs::{current_dir, parent};

mod error;
mod launcher;
mod working_dir;

use error::{
    IcedContext as ErrorContext,
    IcedMessage as ErrorMessage,
};
use launcher::{
    IcedContext as LauncherContext,
    IcedMessage as LauncherMessage,
};
use working_dir::{
    IcedContext as WorkingDirContext,
    IcedMessage as WorkingDirMessage,
};

const DEFAULT_MONO_FONT: Font = Font::with_name("Space Mono");

pub fn run() -> Result<(), Error> {
    iced::application(boot, update, view)
        .theme(Theme::Dark)
        .font(include_bytes!("../../resources/SpaceMono-Regular.ttf"))
        .default_font(DEFAULT_MONO_FONT)
        .subscription(|_| Subscription::batch([
            time::every(Duration::from_millis(1_000)).map(|_| IcedMessage::Tick),
            iced::event::listen().map(|event| match event {
                Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) => IcedMessage::KeyPressed { key, modifiers },
                Event::Window(WindowEvent::Opened { size, .. } | WindowEvent::Resized(size)) => IcedMessage::WindowResized(size),
                _ => IcedMessage::None,
            }),
        ]))
        .run()?;

    Ok(())
}

pub struct IcedContext {
    global: GlobalContext,
    local: LocalContext,
}

pub struct GlobalContext {
    cwd: String,
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
    None,
}

fn boot() -> IcedContext {
    let cwd = current_dir().unwrap();
    let local_context = match launcher::try_boot(None, &cwd) {
        Ok(c) => LocalContext::Launcher(c),
        Err(e) => LocalContext::Error(error::boot(format!("{e:?}"), Size::new(0.0, 0.0), 1.0)),
    };

    IcedContext {
        global: GlobalContext {
            cwd: cwd.to_string(),
        },
        local: local_context,
    }
}

fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
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
            match launcher::try_boot(Some(c.window_size()), &context.global.cwd) {
                Ok(l) => {
                    *c = LocalContext::Launcher(l);
                },
                Err(e) => match parent(&context.global.cwd) {
                    Ok(parent) => match launcher::try_boot(Some(c.window_size()), &parent) {
                        Ok(l) => {
                            context.global.cwd = parent.to_string();
                            *c = LocalContext::Launcher(l);
                        },
                        Err(_) => match std::env::var("HOME") {
                            Ok(home) => match launcher::try_boot(Some(c.window_size()), &home) {
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
                        Ok(home) => match launcher::try_boot(Some(c.window_size()), &home) {
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
        (_, IcedMessage::None) => Task::none(),
        _ => unreachable!(),
    }
}

fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    match &context.local {
        LocalContext::Launcher(c) => launcher::view(c).map(|m| IcedMessage::Launcher(m)),
        LocalContext::WorkingDir(c) => working_dir::view(c).map(|m| IcedMessage::WorkingDir(m)),
        LocalContext::Error(e) => error::view(e).map(|m| IcedMessage::Error(m)),
    }
}

fn button<'s, Message>(name: &'s str, message: Message, bg_color: Color, zoom: f32) -> Button<'s, Message> {
    disabled_button(name, bg_color, zoom).on_press(message)
}

fn disabled_button<'s, Message>(name: &'s str, bg_color: Color, zoom: f32) -> Button<'s, Message> {
    Button::new(text!("{name}").size(zoom * 14.0))
        .style(move |_, status| {
            let (r, g, b) = (bg_color.r, bg_color.g, bg_color.b);
            let bg_color = match status {
                ButtonStatus::Hovered => Color::from_rgba(r, g, b, 0.5),
                _ => bg_color,
            };
            let text_color = if (r > 0.5 && g > 0.5 && b > 0.5) || g > 0.7 || r + g + b > 2.0 {
                black()
            } else {
                white()
            };

            ButtonStyle {
                background: Some(Background::Color(bg_color)),
                text_color: text_color,
                border: Border {
                    color: black(),
                    width: 0.0,
                    radius: Radius::new(6.0),
                },
                ..ButtonStyle::default()
            }
        })
        .padding(8)
}

fn horizontal_bar<'a, Message: 'a>(window_width: f32) -> Element<'a, Message> {
    Container::new(Space::new())
        .style(|_| set_bg(white()))
        .width(Length::Fixed(window_width))
        .height(Length::Fixed(8.0))
        .into()
}

fn set_bg(color: Color) -> Style {
    Style {
        background: Some(Background::Color(color)),
        ..Style::default()
    }
}

fn white() -> Color {
    Color::from_rgb(1.0, 1.0, 1.0)
}

fn black() -> Color {
    Color::from_rgb(0.0, 0.0, 0.0)
}

fn gray(c: f32) -> Color {
    Color::from_rgb(c, c, c)
}

fn red() -> Color {
    Color::from_rgb(0.8, 0.2, 0.2)
}

fn green() -> Color {
    Color::from_rgb(0.2, 0.8, 0.2)
}

fn blue() -> Color {
    Color::from_rgb(0.2, 0.2, 0.8)
}

fn yellow() -> Color {
    Color::from_rgb(0.8, 0.8, 0.2)
}

fn pink() -> Color {
    Color::from_rgb(0.9, 0.6, 0.7)
}
