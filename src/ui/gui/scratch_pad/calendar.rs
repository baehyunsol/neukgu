use super::{black, blue, button, gray, red, set_round_bg, skyblue, white};
use chrono::{Datelike, Local};
use iced::{Element, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Container, MouseArea, Row, Space, text};

pub struct IcedContext {
    pub year: i32,
    pub month: u8,
    pub start_weekday: Weekday,
    pub hovered_id: Option<(i32, u8, u8)>,
    pub today: (i32, u8, u8),
    pub selected: Vec<((i32, u8, u8), Weekday, i64)>,
}

impl IcedContext {
    pub fn new() -> IcedContext {
        let now = Local::now();
        let today = (now.year(), now.month() as u8, now.day() as u8);

        let mut result = IcedContext {
            year: today.0,
            month: today.1,
            start_weekday: Weekday::Monday,
            hovered_id: None,
            today,
            selected: vec![],
        };
        result.calc_weekday();
        result
    }

    pub fn render_month(&self, width: usize) -> String {
        let m = match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => unreachable!(),
        };
        let rem = width.max(m.len()) - m.len();
        format!("{}{m}{}", " ".repeat(rem / 2), " ".repeat(rem / 2 + rem % 2))
    }

    fn clamp(&mut self) -> bool {
        let mut clamped = false;

        if self.year < 1960 {
            self.year = 1960;
            self.month = 1;
            clamped = true;
        }

        else if self.year > 2199 {
            self.year = 2199;
            self.month = 12;
            clamped = true;
        }

        clamped
    }

    fn calc_weekday(&mut self) {
        // VIBE NOTE: gpt 5.5 (via neukgu-chat) taught me this method.
        let days = days_from_civil(self.year, self.month, 1);

        // 1970-01-01 was a Thursday.
        self.start_weekday = match (days + 3).rem_euclid(7) {
            0 => Weekday::Monday,
            1 => Weekday::Tuesday,
            2 => Weekday::Wednesday,
            3 => Weekday::Thursday,
            4 => Weekday::Friday,
            5 => Weekday::Saturday,
            6 => Weekday::Sunday,
            _ => unreachable!(),
        };
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    JumpYear(i32),
    PrevMonth,
    NextMonth,
    Jump(i32, u8),
    Hover(Option<(i32, u8, u8)>),
    Select((i32, u8, u8), Weekday),
    RemoveSelection(usize),
    Today,
    Notify(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub fn from_sunday() -> [Weekday; 7] {
        [
            Weekday::Sunday,
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
        ]
    }

    pub fn short(&self) -> &'static str {
        match self {
            Weekday::Monday => "Mon",
            Weekday::Tuesday => "Tue",
            Weekday::Wednesday => "Wed",
            Weekday::Thursday => "Thu",
            Weekday::Friday => "Fri",
            Weekday::Saturday => "Sat",
            Weekday::Sunday => "Sun",
        }
    }
}

impl From<Weekday> for u8 {
    fn from(w: Weekday) -> u8 {
        match w {
            Weekday::Monday => 0,
            Weekday::Tuesday => 1,
            Weekday::Wednesday => 2,
            Weekday::Thursday => 3,
            Weekday::Friday => 4,
            Weekday::Saturday => 5,
            Weekday::Sunday => 6,
        }
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    let mut clamped = false;

    match message {
        IcedMessage::JumpYear(d) => {
            context.year += d;
            clamped = context.clamp();
            context.calc_weekday();
        },
        IcedMessage::PrevMonth => {
            if context.month == 1 {
                context.month = 12;
                context.year -= 1;
            }

            else {
                context.month -= 1;
            }

            clamped = context.clamp();
            context.calc_weekday();
        },
        IcedMessage::NextMonth => {
            if context.month == 12 {
                context.month = 1;
                context.year += 1;
            }

            else {
                context.month += 1;
            }

            clamped = context.clamp();
            context.calc_weekday();
        },
        IcedMessage::Jump(y, m) => {
            context.year = y;
            context.month = m;
            clamped = context.clamp();
            context.calc_weekday();
        },
        IcedMessage::RemoveSelection(i) => {
            context.selected.remove(i);
        },
        IcedMessage::Hover(id) => {
            context.hovered_id = id;
        },
        IcedMessage::Select(id, weekday) => {
            context.selected.push((id, weekday, days_from_civil(id.0, id.1, id.2)));
        },
        IcedMessage::Today => {
            let (year, month, _) = context.today;
            context.year = year;
            context.month = month;
            clamped = context.clamp();
            context.calc_weekday();
        },
        IcedMessage::Notify(_) => unreachable!(),
    }

    if clamped {
        Task::done(IcedMessage::Notify(String::from("Only 1960 ~ 2199 is available.")))
    }

    else {
        Task::none()
    }
}

pub fn view<'c>(context: &'c IcedContext, window_size: Size, zoom: f32) -> Element<'c, IcedMessage> {
    Column::from_vec(vec![
        Row::from_vec(vec![
            button("<<", IcedMessage::JumpYear(-10), blue(), zoom).into(),
            button(" < ", IcedMessage::JumpYear(-1), blue(), zoom).into(),
            text!("    {:>5}     ", context.year).color(black()).size(zoom * 14.0).into(),
            button(" > ", IcedMessage::JumpYear(1), blue(), zoom).into(),
            button(">>", IcedMessage::JumpYear(10), blue(), zoom).into(),
        ])
            .spacing(zoom * 8.0)
            .align_y(Vertical::Center)
            .into(),
        Row::from_vec(vec![
            button(" < ", IcedMessage::PrevMonth, blue(), zoom).into(),
            text!("{}", context.render_month(14)).color(black()).size(zoom * 14.0).into(),
            button(" > ", IcedMessage::NextMonth, blue(), zoom).into(),
        ])
            .spacing(zoom * 8.0)
            .align_y(Vertical::Center)
            .into(),
        render_calendar(context, zoom),
        button("Today", IcedMessage::Today, blue(), zoom).into(),
        render_selections(&context.selected, zoom),
        Space::new().width(window_size.width).into(),
    ])
        .spacing(zoom * 8.0)
        .align_x(Horizontal::Center)
        .into()
}

fn render_calendar<'c>(context: &'c IcedContext, zoom: f32) -> Element<'c, IcedMessage> {
    fn render_cell<'c>(
        cell_id: (i32, u8, u8),
        hovered_id: Option<(i32, u8, u8)>,
        activated: bool,
        day: u8,
        today: (i32, u8, u8),
        weekday: Weekday,
        zoom: f32,
    ) -> Element<'c, IcedMessage> {
        let bg_color = if !activated {
            gray(0.6)
        } else if Some(cell_id) == hovered_id {
            gray(0.4)
        } else {
            gray(0.2)
        };
        let color = match weekday {
            Weekday::Sunday => red(),
            Weekday::Saturday => skyblue(),
            _ => white(),
        };

        let mut m = MouseArea::new(
            Container::new(Column::from_vec(vec![
                text!(
                    "{day:>2}{}   ",
                    if cell_id == today { " @" } else { "  " },
                ).color(color).size(zoom * 12.0).into(),
                Space::new().height(zoom * 32.0).into(),
            ]))
                .padding(zoom * 4.0)
                .style(move |_| set_round_bg(bg_color, zoom))
        );

        if activated {
            m = m
                .on_enter(IcedMessage::Hover(Some(cell_id)))
                .on_exit(IcedMessage::Hover(None))
                .on_press(IcedMessage::Select(cell_id, weekday));
        }

        m.into()
    }

    let mut column: Vec<Element<IcedMessage>> = vec![];
    let (mut year, mut month, mut day) = (context.year, context.month, 1);

    // The calendar starts with Sunday.
    // It shows 2 more weeks before and after the month.
    for _ in 0..(14 + (u8::from(context.start_weekday) + 1) % 7) {
        let (y, m, d) = prev_day(year, month, day);
        year = y;
        month = m;
        day = d;
    }

    for _ in 0..9 {
        let mut row: Vec<Element<IcedMessage>> = vec![];

        for w in Weekday::from_sunday() {
            row.push(render_cell(
                (year, month, day),
                context.hovered_id,
                context.month == month,
                day,
                context.today,
                w,
                zoom,
            ));
            let (y, m, d) = next_day(year, month, day);
            year = y;
            month = m;
            day = d;
        }

        column.push(Row::from_vec(row).spacing(zoom * 4.0).into());
    }

    Column::from_vec(column).spacing(zoom * 4.0).into()
}

fn render_selections<'s>(selections: &'s [((i32, u8, u8), Weekday, i64)], zoom: f32) -> Element<'s, IcedMessage> {
    Column::from_vec(selections.iter().enumerate().map(
        |(i, ((y, m, d), weekday, count))| {
            Row::from_vec(vec![
                text!("{y}-{m:02}-{d:02}, {} (day {count})", weekday.short()).color(black()).size(zoom * 14.0).into(),
                button("Jump", IcedMessage::Jump(*y, *m), blue(), zoom).into(),
                button("Remove", IcedMessage::RemoveSelection(i), red(), zoom).into(),
            ])
                .align_y(Vertical::Center)
                .spacing(zoom * 8.0)
                .into()
        }
    ).collect())
        .spacing(zoom * 8.0)
        .into()
}

// VIBE NOTE: gpt 5.5 (via neukgu-chat) wrote this function and I'm not sure if it's correct.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let mut y = year as i64;
    let m = month as i64;
    let d = day as i64;

    // Treat March as the first month of the year.
    if m <= 2 {
        y -= 1;
    }

    let era = y.div_euclid(400);
    let yoe = y - era * 400; // year of era: [0, 399]

    let mp = m + if m > 2 { -3 } else { 9 }; // March = 0, ..., February = 11
    let doy = (153 * mp + 2) / 5 + d - 1; // day of year: [0, 365]

    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era

    // Days since 1970-01-01.
    era * 146_097 + doe - 719_468
}

fn prev_day(year: i32, month: u8, day: u8) -> (i32, u8, u8) {
    match (month, day) {
        (1, 1) => (year - 1, 12, 31),
        (2 | 4 | 6 | 8 | 9 | 11, 1) => (year, month - 1, 31),
        (3, 1) if is_leap_year(year) => (year, 2, 29),
        (3, 1) => (year, 2, 28),
        (5 | 7 | 10 | 12, 1) => (year, month - 1, 30),
        _ => (year, month, day - 1),
    }
}

fn next_day(year: i32, month: u8, day: u8) -> (i32, u8, u8) {
    match (month, day) {
        (1 | 3 | 5 | 7 | 8 | 10, 31) => (year, month + 1, 1),
        (2, 28) if is_leap_year(year) => (year, 2, 29),
        (2, 28) => (year, 3, 1),
        (2, 29) => (year, 3, 1),
        (4 | 6 | 9 | 11, 30) => (year, month + 1, 1),
        (12, 31) => (year + 1, 1, 1),
        _ => (year, month, day + 1),
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
