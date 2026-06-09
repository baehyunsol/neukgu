use super::{button, gray, set_round_bg, yellow};
use super::popup::{PopupContext, PopupMessage, into_popup};
use crate::{LogId, Model, TokenUsage, prettify_tokens};
use iced::{Element, Length};
use iced::alignment::Vertical;
use iced::widget::{Column, Container, Id, Row, Scrollable, text};
use regex::Regex;
use std::collections::HashMap;
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

pub fn render_token_usage<'u, 'c, Context: PopupContext, Message: Clone + PopupMessage + 'c>(
    token_usage: &'u TokenUsage,
    context: &'c Context,
    scroll_id: Id,
    zoom: f32,
) -> Element<'c, Message> {
    if token_usage.is_empty() {
        into_popup(
            text!("You haven't used any tokens.").size(zoom * 14.0).into(),
            context,
        )
    }

    else {
        let total = token_usage.total();
        let recent = token_usage.recent();

        into_popup(
            Scrollable::new(
                Column::from_vec(vec![
                    render_each_token_usage("Total", total, zoom),
                    render_each_token_usage("Recent 6 hours", recent, zoom),
                ]).spacing(zoom * 12.0)
            ).id(scroll_id).into(),
            context,
        )
    }
}

fn render_each_token_usage<'c, Message: 'c>(
    title: &'static str,
    token_usage: HashMap<Model, (u64, u64, u64)>,
    zoom: f32,
) -> Element<'c, Message> {
    let mut token_usage: Vec<(Model, (u64, u64, u64))> = token_usage.into_iter().collect();
    token_usage.sort_by_key(|(m, _)| *m);
    let mut column: Vec<Element<Message>> = vec![text!("{title}").size(zoom * 18.0).into()];

    for (model, (cached_input, input, output)) in token_usage.iter() {
        column.push(
            Container::new(
                Column::from_vec(vec![
                    text!("----- {} -----", model.short_name()).size(zoom * 14.0).into(),
                    text!("cached_input: {}", prettify_tokens(*cached_input)).size(zoom * 14.0).into(),
                    text!("       input: {}", prettify_tokens(*input)).size(zoom * 14.0).into(),
                    text!("      output: {}", prettify_tokens(*output)).size(zoom * 14.0).into(),
                ])
                    .spacing(zoom * 4.0)
            )
                .padding(zoom * 8.0)
                .style(move |_| set_round_bg(gray(0.25), zoom))
                .into()
        );
    }

    Column::from_vec(column).spacing(zoom * 8.0).into()
}
