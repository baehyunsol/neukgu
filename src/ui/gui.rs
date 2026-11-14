use super::{FeContext, Truncation, spawn_be_process};
use crate::Error;
use iced::{Background, Color, Element, Event, Font, Subscription, Theme};
use iced::border::{Border, Radius};
use iced::keyboard::Event as KeyboardEvent;
use iced::time::{self, Duration};
use iced::widget::{Space, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::checkbox::{Status as CheckboxStatus, Style as CheckboxStyle};
use iced::widget::container::{Container, Style};
use iced::widget::radio::{Status as RadioStatus, Style as RadioStyle};
use iced::widget::slider::{
    Handle as SliderHandle,
    HandleShape as SliderHandleShape,
    Rail as SliderRail,
    Status as SliderStatus,
    Style as SliderStyle,
};
use iced::widget::text_input::{Status as TextInputStatus, Style as TextInputStyle};
use iced::window::Event as WindowEvent;
use std::sync::atomic::{AtomicBool, Ordering};

mod api_key;
mod browser;
mod chat;
mod config;
mod error;
mod file_change;
mod file_editor;
mod file_selector;
mod git;
mod index;
mod logs;
mod model_store;
mod popup;
mod scratch_pad;
mod tab;
mod tabs;
mod worker;
mod working_dir;

use tabs::IcedMessage as TabsMessage;

// widget::TextEditor is very heavy, especially with syntax highlighting, so we have to limit the size of the content.
// NOTE: The limit must never be less than 680.
const TEXT_EDITOR_CONTENT_LIMIT: usize = 65536;

// If this limit is too small, the file browser will fail to load large image files (instead it'll just dump the hex).
// If this limit is too big, the file browser will take very long time to dump the hex of large binary files.
// NOTE: This limit must be greater than `TEXT_EDITOR_CONTENT_LIMIT`.
const MEMORY_LOAD_LIMIT: usize = 33554432;

const DEFAULT_MONO_FONT: Font = Font::with_name("Space Mono");

pub fn run(cwd: Option<String>) -> Result<(), Error> {
    iced::application(move || tabs::boot(&cwd), tabs::update, tabs::view)
        .font(include_bytes!("../../resources/SpaceMono-Bold.ttf"))
        .font(include_bytes!("../../resources/SpaceMono-BoldItalic.ttf"))
        .font(include_bytes!("../../resources/SpaceMono-Italic.ttf"))
        .font(include_bytes!("../../resources/SpaceMono-Regular.ttf"))
        .default_font(DEFAULT_MONO_FONT)
        .subscription(|_| Subscription::batch([
            time::every(Duration::from_millis(100)).map(|_| TabsMessage::Tick),
            iced::event::listen().map(|event| match event {
                Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) => TabsMessage::KeyPressed { key, modifiers },
                Event::Window(WindowEvent::Opened { size, .. } | WindowEvent::Resized(size)) => TabsMessage::WindowResized(size),
                _ => TabsMessage::None,
            }),
        ]))
        .run()?;

    Ok(())
}

fn button<'s, 'm, Message>(name: &'s str, message: Message, bg_color: Color, zoom: f32) -> Button<'m, Message> {
    disabled_button(name, bg_color, zoom).on_press(message)
}

fn disabled_button<'s, 'm, Message>(name: &'s str, bg_color: Color, zoom: f32) -> Button<'m, Message> {
    Button::new(text!("{name}").size(zoom * 14.0))
        .style(move |_, status| {
            let (r, g, b) = (bg_color.r, bg_color.g, bg_color.b);
            let bg_color = match status {
                ButtonStatus::Hovered => Color::from_rgba(r, g, b, 0.5),
                _ => bg_color,
            };
            let invert_color = INVERT_COLOR.load(Ordering::Relaxed);
            let text_color = if ((r > 0.5 && g > 0.5 && b > 0.5) || g > 0.7 || r + g + b > 2.0) ^ invert_color {
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
                    radius: Radius::new(zoom * 6.0),
                },
                ..ButtonStyle::default()
            }
        })
        .padding(zoom * 8.0)
}

fn radio_style(_: &Theme, status: RadioStatus) -> RadioStyle {
    let (background, text_color) = match status {
        RadioStatus::Active { .. } => (Background::Color(white()), Some(white())),
        RadioStatus::Hovered { .. } => (Background::Color(white_transparent()), Some(white_transparent())),
    };

    RadioStyle {
        background,
        dot_color: purple(),
        border_width: 1.0,
        border_color: black(),
        text_color,
    }
}

fn checkbox_style(_: &Theme, status: CheckboxStatus) -> CheckboxStyle {
    match status {
        CheckboxStatus::Active { .. } => CheckboxStyle {
            background: Background::Color(purple()),
            icon_color: white(),
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(4.0),
            },
            text_color: Some(white()),
        },
        CheckboxStatus::Hovered { .. } => CheckboxStyle {
            background: Background::Color(purple_transparent()),
            icon_color: white_transparent(),
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(4.0),
            },
            text_color: Some(white_transparent()),
        },
        CheckboxStatus::Disabled { .. } => CheckboxStyle {
            background: Background::Color(gray_transparent()),
            icon_color: white(),
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(6.0),
            },
            text_color: Some(white()),
        },
    }
}

fn slider_style(status: SliderStatus, zoom: f32) -> SliderStyle {
    let handle_background = match status {
        SliderStatus::Active => gray(0.5),
        SliderStatus::Hovered | SliderStatus::Dragged => white(),
    };

    SliderStyle {
        rail: SliderRail {
            backgrounds: (Background::Color(purple()), Background::Color(gray(0.5))),
            width: zoom * 4.0,
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(0.0),
            },
        },
        handle: SliderHandle {
            shape: SliderHandleShape::Circle { radius: zoom * 8.0 },
            background: Background::Color(handle_background),
            border_width: 0.0,
            border_color: white(),
        },
    }
}

fn text_input_style(status: TextInputStatus, zoom: f32) -> TextInputStyle {
    let (background, border_color, border_width) = match status {
        TextInputStatus::Active => (gray(0.3), gray(0.7), zoom * 2.0),
        TextInputStatus::Hovered => (gray(0.3), white(), zoom * 2.0),
        TextInputStatus::Focused { .. } => (gray(0.25), purple(), zoom * 3.0),
        TextInputStatus::Disabled => (black(), black(), 0.0),
    };

    TextInputStyle {
        background: Background::Color(background),
        border: Border {
            color: border_color,
            width: border_width,
            radius: Radius::new(0.0),
        },
        icon: yellow(),  // what's this?
        placeholder: gray(0.6),
        value: white(),
        selection: purple(),
    }
}

fn circle<'m, Message: 'm>(radius: f32, color: Color) -> Element<'m, Message> {
    Container::new(Space::new()).padding(radius).style(move |_| Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: black(),
            width: 0.0,
            radius: Radius::new(radius),
        },
        ..Style::default()
    }).into()
}

fn set_round_bg(color: Color, zoom: f32) -> Style {
    Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: white(),
            width: 0.0,
            radius: Radius::new(zoom * 8.0),
        },
        ..Style::default()
    }
}

fn set_bg(color: Color) -> Style {
    Style {
        background: Some(Background::Color(color)),
        ..Style::default()
    }
}

static INVERT_COLOR: AtomicBool = AtomicBool::new(false);

fn invert_color(c: Color) -> Color {
    if INVERT_COLOR.load(Ordering::Relaxed) {
        c.inverse()
    } else {
        c
    }
}

fn white() -> Color {
    invert_color(Color::from_rgb(1.0, 1.0, 1.0))
}

fn white_transparent() -> Color {
    invert_color(Color::from_rgba(1.0, 1.0, 1.0, 0.5))
}

fn black() -> Color {
    invert_color(Color::from_rgb(0.0, 0.0, 0.0))
}

fn gray(c: f32) -> Color {
    invert_color(Color::from_rgb(c, c, c))
}

fn gray_transparent() -> Color {
    invert_color(Color::from_rgba(0.5, 0.5, 0.5, 0.5))
}

fn red() -> Color {
    invert_color(Color::from_rgb(0.8, 0.2, 0.2))
}

fn red_transparent() -> Color {
    invert_color(Color::from_rgba(0.8, 0.2, 0.2, 0.5))
}

fn green() -> Color {
    invert_color(Color::from_rgb(0.2, 0.8, 0.2))
}

fn green_transparent() -> Color {
    invert_color(Color::from_rgba(0.2, 0.8, 0.2, 0.5))
}

fn blue() -> Color {
    invert_color(Color::from_rgb(0.2, 0.2, 0.8))
}

fn yellow() -> Color {
    invert_color(Color::from_rgb(0.8, 0.8, 0.2))
}

fn brown() -> Color {
    invert_color(Color::from_rgb(0.5, 0.3, 0.2))
}

fn pink() -> Color {
    invert_color(Color::from_rgb(0.9, 0.6, 0.7))
}

fn purple() -> Color {
    invert_color(Color::from_rgb(0.7, 0.5, 0.8))
}

fn purple_transparent() -> Color {
    invert_color(Color::from_rgba(0.7, 0.5, 0.8, 0.5))
}

fn skyblue() -> Color {
    invert_color(Color::from_rgb(0.5, 0.7, 0.9))
}

fn gold() -> Color {
    invert_color(Color::from_rgb(0.9, 0.8, 0.1))
}

fn count_chars(s: &str) -> usize {
    s.chars().map(
        |ch| match ch {
            '가'..='힣' => 10,
            _ => 7,
        }
    ).sum::<usize>() / 7
}

fn take_chars(s: &str, n: usize) -> String {
    let mut weight = 0;
    let mut chars = vec![];

    for ch in s.chars() {
        weight += match ch {
            '가'..='힣' => 10,
            _ => 7,
        };

        if weight > n * 7 {
            break;
        }

        chars.push(ch);
    }

    chars.into_iter().collect()
}

