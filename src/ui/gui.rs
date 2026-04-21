use super::{FeContext, Truncation, spawn_backend_process};
use crate::Error;
use iced::{Background, Color, Element, Font, Length, Subscription, Task, Theme};
use iced::border::{Border, Radius};
use iced::keyboard::{self, Event as KeyboardEvent, Key, key::Named as NamedKey};
use iced::time::{self, Duration};
use iced::widget::{Container, Space};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::Style;

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

pub fn run() -> Result<(), Error> {
    iced::application(boot, update, view)
        .theme(Theme::Dark)
        .default_font(Font::MONOSPACE)
        .subscription(|_| Subscription::batch([
            time::every(Duration::from_millis(1_000)).map(|_| IcedMessage::Tick),
            keyboard::listen().map(|key| match key {
                KeyboardEvent::KeyPressed { key: Key::Named(NamedKey::Escape), .. } => IcedMessage::PressedEscKey,
                _ => IcedMessage::None,
            }),
        ]))
        .run()?;

    Ok(())
}

pub enum IcedContext {
    Launcher(LauncherContext),
    WorkingDir(WorkingDirContext),
    Error(ErrorContext),
}

pub enum IcedMessage {
    Launcher(LauncherMessage),
    WorkingDir(WorkingDirMessage),
    Error(ErrorMessage),
    Tick,
    PressedEscKey,
    None,
}

fn boot() -> IcedContext {
    match launcher::try_boot() {
        Ok(c) => IcedContext::Launcher(c),
        Err(e) => IcedContext::Error(error::boot(format!("{e:?}"))),
    }
}

fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match (context, message) {
        (context, IcedMessage::Launcher(LauncherMessage::Error(e)) | IcedMessage::WorkingDir(WorkingDirMessage::Error(e))) => {
            *context = IcedContext::Error(error::boot(e));
            Task::none()
        },
        (context, IcedMessage::Launcher(LauncherMessage::Launch { path })) => {
            // TODO: make `no_backend` configurable
            match working_dir::try_boot(false, &path) {
                Ok(c) => {
                    *context = IcedContext::WorkingDir(c);
                },
                Err(e) => {
                    *context = IcedContext::Error(error::boot(format!("{e:?}")));
                },
            }

            Task::none()
        },
        (IcedContext::Launcher(c), IcedMessage::Launcher(m)) => launcher::update(c, m).map(|t| IcedMessage::Launcher(t)),
        (IcedContext::WorkingDir(c), IcedMessage::Tick) => working_dir::update(c, WorkingDirMessage::Tick).map(|t| IcedMessage::WorkingDir(t)),
        (IcedContext::WorkingDir(c), IcedMessage::PressedEscKey) => working_dir::update(c, WorkingDirMessage::ClosePopup).map(|t| IcedMessage::WorkingDir(t)),
        (IcedContext::WorkingDir(c), IcedMessage::WorkingDir(m)) => working_dir::update(c, m).map(|t| IcedMessage::WorkingDir(t)),
        (context, IcedMessage::Error(ErrorMessage::Okay)) => {
            match launcher::try_boot() {
                Ok(c) => {
                    *context = IcedContext::Launcher(c);
                },
                Err(e) => {
                    *context = IcedContext::Error(error::boot(format!("{e:?}")));
                },
            }

            Task::none()
        },
        (IcedContext::Error(c), IcedMessage::Error(m)) => error::update(c, m).map(|t| IcedMessage::Error(t)),
        (_, IcedMessage::Tick | IcedMessage::PressedEscKey | IcedMessage::None) => Task::none(),
        _ => todo!(),
    }
}

fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    match context {
        IcedContext::Launcher(c) => launcher::view(c).map(|m| IcedMessage::Launcher(m)),
        IcedContext::WorkingDir(c) => working_dir::view(c).map(|m| IcedMessage::WorkingDir(m)),
        IcedContext::Error(e) => error::view(e).map(|m| IcedMessage::Error(m)),
    }
}

fn button<'s, Message>(name: &'s str, message: Message, solid_color: Color) -> Button<'s, Message> {
    Button::new(name)
        .style(move |_, status| {
            let bg_color = match status {
                ButtonStatus::Hovered => Color::from_rgba(solid_color.r, solid_color.g, solid_color.b, 0.5),
                _ => solid_color,
            };

            ButtonStyle {
                background: Some(Background::Color(bg_color)),
                text_color: white(),
                border: Border {
                    color: black(),
                    width: 0.0,
                    radius: Radius::new(4.0),
                },
                ..ButtonStyle::default()
            }
        })
        .padding(8)
        .on_press(message)
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
