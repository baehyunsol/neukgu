use super::{button, yellow};
use super::popup::{PopupContext, PopupMessage, into_popup};
use crate::LogId;
use iced::{Element, Length};
use iced::alignment::Vertical;
use iced::widget::{Column, Id, Row, Scrollable, text};
use regex::Regex;
use std::sync::LazyLock;

pub static LOG_DETAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*\((\d{7}\-\d{7})\).*").unwrap());

pub trait LogsContext {
    type Message;
    fn open_log_popup(&self, log_title: String, log_id: LogId) -> Self::Message;
}

pub fn render_logs<'l, 'c, Context: LogsContext<Message=Message> + PopupContext, Message: Clone + PopupMessage + 'c>(
    logs: &'l [String],
    context: &'c Context,
    scroll_id: Id,
    zoom: f32,
) -> Element<'c, Message> {
    let logs = Scrollable::new(
        Column::from_vec(
            logs.iter().map(
                |log| {
                    if let Some(cap) = LOG_DETAIL_RE.captures(log) {
                        let log_title = cap.get(0).unwrap().as_str().to_string();
                        let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                        Row::from_vec(vec![
                            text!("{log}").size(zoom * 14.0).into(),
                            button("see details", context.open_log_popup(log_title, log_id), yellow(), zoom).into(),
                        ]).align_y(Vertical::Center).spacing(zoom * 20.0).into()
                    }

                    else {
                        text!("{log}").size(zoom * 14.0).into()
                    }
                }
            ).collect()
        )
            .padding(zoom * 8.0)
            .spacing(zoom * 8.0)
            .width(Length::Fill)
    ).id(scroll_id.clone()).width(Length::Fill);
    into_popup(logs.into(), context)
}
