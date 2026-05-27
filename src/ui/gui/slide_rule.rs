use super::{black, gold};
use iced::{Background, Color, Element, Size, Task};
use iced::alignment::Horizontal;
use iced::widget::{Column, Row, Slider, Space, text};
use iced::widget::radio::{
    Radio,
    Status as RadioStatus,
    Style as RadioStyle,
};

const SCALE: i32 = 18;
const CONTEXT_SIZE: i32 = 5;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub parent_kind: RulerKind,
    pub child_kind: RulerKind,
    pub child_coarse: i32,
    pub child_fine: i32,
    pub cursor_coarse: i32,
    pub cursor_fine: i32,
}

impl IcedContext {
    pub fn new() -> IcedContext {
        IcedContext {
            parent_kind: RulerKind::Log10,
            child_kind: RulerKind::Log10,
            child_coarse: 0,
            child_fine: 0,
            cursor_coarse: 0,
            cursor_fine: 0,
        }
    }

    pub fn render(&self) -> (String, f64, f64) {
        let child_offset = self.child_coarse * SCALE + self.child_fine;
        let cursor = self.cursor_coarse * SCALE + self.cursor_fine;
        let mut lines = Vec::with_capacity(CONTEXT_SIZE as usize * 2 + 1);
        let mut pointed_value = (0.0, 0.0);

        for i in -CONTEXT_SIZE..(CONTEXT_SIZE + 1) {
            let delim = match (child_offset + cursor + i).abs() % 4 {
                0 => " | ",
                1 | 3 => "   ",
                _ => " : ",
            };
            let parent_value = self.parent_kind.value(0, SCALE * SCALE, child_offset + cursor + i);
            let parent_str = format!("-- {} --", render_f64(parent_value));

            let child_value = self.child_kind.value(0, SCALE * SCALE, cursor + i);
            let child_str = if cursor + i < 0 || cursor + i >= SCALE * SCALE {
                String::new()
            } else {
                format!("-- {} --", render_f64(child_value))
            };

            let cursor_str = if i == 0 {
                String::from(" <--")
            } else {
                String::new()
            };

            lines.push(format!("   {parent_str}{delim}{child_str}{cursor_str}"));

            if i == 0 {
                pointed_value = (parent_value, child_value);
            }
        }

        (lines.join("\n"), pointed_value.0, pointed_value.1)
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    ParentKind(RulerKind),
    ChildKind(RulerKind),
    ChildCoarse(i32),
    ChildFine(i32),
    CursorCoarse(i32),
    CursorFine(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RulerKind {
    Log10,
    Linear,
}

impl RulerKind {
    pub fn value(&self, start: i32, end: i32, cursor: i32) -> f64 {
        let n = (cursor - start) as f64 / (end - start) as f64;
        match self {
            RulerKind::Log10 => 10.0f64.powf(n),
            RulerKind::Linear => n,
        }
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::ParentKind(k) => {
            context.parent_kind = k;
        },
        IcedMessage::ChildKind(k) => {
            context.child_kind = k;
        },
        IcedMessage::ChildCoarse(v) => {
            context.child_coarse = v;
        },
        IcedMessage::ChildFine(v) => {
            context.child_fine = v;
        },
        IcedMessage::CursorCoarse(v) => {
            context.cursor_coarse = v;
        },
        IcedMessage::CursorFine(v) => {
            context.cursor_fine = v;
        },
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext, window_size: Size, zoom: f32) -> Element<'c, IcedMessage> {
    fn radio<F>(
        label: &'static str,
        value: RulerKind,
        selected: Option<RulerKind>,
        f: F,
        zoom: f32,
    ) -> Radio<'static, IcedMessage> where F: FnOnce(RulerKind) -> IcedMessage {
        Radio::new(label, value, selected, f)
            .spacing(zoom * 4.0)
            .text_size(zoom * 14.0)
            .size(zoom * 14.0)
            .style(|_, status| {
                let background = match status {
                    RadioStatus::Hovered { is_selected: false } => gold(),
                    _ => Color::from_rgba(0.0, 0.0, 0.0, 0.0),
                };

                RadioStyle {
                    background: Background::Color(background),
                    dot_color: gold(),
                    border_width: 1.0,
                    border_color: black(),
                    text_color: Some(black()),
                }
            })
    }

    let (ruler, pv0, pv1) = context.render();
    Column::from_vec(vec![
        Row::from_vec(vec![
            text!(" left: ").color(black()).size(zoom * 14.0).into(),
            radio("log", RulerKind::Log10, Some(context.parent_kind), IcedMessage::ParentKind, zoom).into(),
            radio("linear", RulerKind::Linear, Some(context.parent_kind), IcedMessage::ParentKind, zoom).into(),
        ]).spacing(zoom * 8.0).into(),
        Row::from_vec(vec![
            text!("right: ").color(black()).size(zoom * 14.0).into(),
            radio("log", RulerKind::Log10, Some(context.child_kind), IcedMessage::ChildKind, zoom).into(),
            radio("linear", RulerKind::Linear, Some(context.child_kind), IcedMessage::ChildKind, zoom).into(),
        ]).spacing(zoom * 8.0).into(),
        text!("add: {}, sub: {}", render_f64(pv0 + pv1), render_f64(pv0 - pv1)).color(black()).size(zoom * 14.0).into(),
        text!("{ruler}").color(black()).size(zoom * 14.0).into(),
        Space::new().height(zoom * 4.0).into(),
        Slider::new(0..=(SCALE - 1), context.child_coarse, IcedMessage::ChildCoarse).width(zoom * SCALE as f32 * 16.0).into(),
        Space::new().height(zoom * 4.0).into(),
        Slider::new(0..=(SCALE - 1), context.child_fine, IcedMessage::ChildFine).width(zoom * SCALE as f32 * 16.0).into(),
        Space::new().height(zoom * 4.0).into(),
        Slider::new(0..=(SCALE - 1), context.cursor_coarse, IcedMessage::CursorCoarse).width(zoom * SCALE as f32 * 16.0).into(),
        Space::new().height(zoom * 4.0).into(),
        Slider::new(0..=(SCALE - 1), context.cursor_fine, IcedMessage::CursorFine).width(zoom * SCALE as f32 * 16.0).into(),
    ])
        .align_x(Horizontal::Center)
        .spacing(zoom * 4.0)
        .width(window_size.width)
        .into()
}

fn render_f64(n: f64) -> String {
    if n <= -100.0 {
        format!("{n:.1}")
    } else if n <= -10.0 {
        format!("{n:.2}")
    } else if n < 0.0 {
        format!("{n:.3}")
    } else if n < 10.0 {
        format!("{n:.4}")
    } else if n < 100.0 {
        format!("{n:.3}")
    } else {
        format!("{n:.2}")
    }
}
