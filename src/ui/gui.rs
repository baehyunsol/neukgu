use super::{FeContext, Truncation, spawn_be_process};
use crate::Error;
use iced::{Background, Color, Event, Font, Subscription, Theme};
use iced::border::{Border, Radius};
use iced::keyboard::Event as KeyboardEvent;
use iced::time::{self, Duration};
use iced::widget::text;
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::Style;
use iced::window::Event as WindowEvent;

mod browser;
mod error;
mod index;
mod tab;
mod tabs;
mod working_dir;

use tabs::IcedMessage as TabsMessage;

const DEFAULT_MONO_FONT: Font = Font::with_name("Space Mono");

pub fn run() -> Result<(), Error> {
    iced::application(tabs::boot, tabs::update, tabs::view)
        .theme(Theme::Dark)
        .font(include_bytes!("../../resources/SpaceMono-Regular.ttf"))
        .default_font(DEFAULT_MONO_FONT)
        .subscription(|_| Subscription::batch([
            time::every(Duration::from_millis(1_000)).map(|_| TabsMessage::Tick),
            iced::event::listen().map(|event| match event {
                Event::Keyboard(KeyboardEvent::KeyPressed { key, modifiers, .. }) => TabsMessage::KeyPressed { key, modifiers },
                Event::Window(WindowEvent::Opened { size, .. } | WindowEvent::Resized(size)) => TabsMessage::WindowResized(size),
                _ => TabsMessage::None,
            }),
        ]))
        .run()?;

    Ok(())
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
