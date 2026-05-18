use super::{gray, set_round_bg};
use crate::{Config, Model};
use iced::Element;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Container, Radio, Row, Slider, text};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Questionable {
    Always,
    Maybe,
    Never,
}

#[derive(Clone, Debug)]
pub enum SetProjectConfig {
    BigAgent(Model),
    SmallAgent(Model),
    SearchAgent(Model),
    SummaryAgent(Model),
    AgentQuestionable(Questionable),
    ContextSize(u64),
}

pub fn set_project_config(config: &mut Config, set: SetProjectConfig) {
    match set {
        SetProjectConfig::BigAgent(m) => {
            config.agents.big = m;
        },
        SetProjectConfig::SmallAgent(m) => {
            config.agents.small = m;
        },
        SetProjectConfig::SearchAgent(m) => {
            config.agents.search = m;
        },
        SetProjectConfig::SummaryAgent(m) => {
            config.agents.summary = m;
        },
        SetProjectConfig::AgentQuestionable(q) => {
            let timeout = match q {
                Questionable::Never => 0,
                Questionable::Always => 999_999,
                Questionable::Maybe => 300,
            };

            config.user_response_timeout = timeout;
        },
        SetProjectConfig::ContextSize(n) => {
            config.llm_context_max_len = n * 65536;
            config.text_file_max_len = n * 8192;
            config.text_file_max_lines = n * 128;
            // config.pdf_max_pages = _;  // TODO
            config.dir_max_entries = n * 128;
            config.stdout_max_len = n * 1280;
        },
    }
}

pub fn config_ui<'c, 'm>(config: &'c Config, zoom: f32) -> Element<'m, SetProjectConfig> {
    let mut panels = vec![];
    let agent_panels: Vec<(&str, fn(&Model) -> bool, Model, fn(Model) -> SetProjectConfig)> = vec![
        ("    Big Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.big, SetProjectConfig::BigAgent),
        ("  Small Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.small, SetProjectConfig::SmallAgent),
        (" Search Agent: ", |m| m.supports_web_search(), config.agents.search, SetProjectConfig::SearchAgent),
        ("Summary Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.summary, SetProjectConfig::SummaryAgent),
    ];

    for (title, filter, state, message) in agent_panels.into_iter() {
        let models: Vec<Model> = Model::all().into_iter().filter(filter).collect();
        let radios = if models.len() < 4 {
            Row::from_vec(models.into_iter().map(
                |m| Radio::new(m.short_name(), m, Some(state), message)
                    .spacing(zoom * 8.0)
                    .text_size(zoom * 14.0)
                    .size(zoom * 14.0)
                    .into()
            ).collect()).spacing(zoom * 16.0).into()
        } else {
            Column::from_vec(models.chunks(4).map(
                |models| Row::from_vec(models.to_vec().into_iter().map(
                    |m| Radio::new(m.short_name(), m, Some(state), message)
                        .spacing(zoom * 8.0)
                        .text_size(zoom * 14.0)
                        .size(zoom * 14.0)
                        .into()
                ).collect()).spacing(zoom * 16.0).into()
            ).collect()).spacing(zoom * 8.0).into()
        };

        panels.push(Container::new(
            Row::from_vec(vec![text!("{title}").size(zoom * 14.0).into(), radios]).align_y(Vertical::Center)
        ).style(move |_| set_round_bg(gray(0.15), zoom)).padding(zoom * 8.0).into());
    }

    let curr_q = match config.user_response_timeout {
        0 => Questionable::Never,
        999_999 => Questionable::Always,
        _ => Questionable::Maybe,
    };
    panels.push(Container::new(Column::from_vec(vec![
        text!("Will you answer neukgu's questions?").size(zoom * 14.0).into(),
        Row::from_vec(vec![
            Radio::new("Always", Questionable::Always, Some(curr_q), SetProjectConfig::AgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("If I'm available", Questionable::Maybe, Some(curr_q), SetProjectConfig::AgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Never", Questionable::Never, Some(curr_q), SetProjectConfig::AgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
        ]).spacing(zoom * 16.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0)).style(move |_| set_round_bg(gray(0.15), zoom)).padding(zoom * 8.0).into());

    panels.push(Container::new(Column::from_vec(vec![
        text!("Context size: {} KiB", config.llm_context_max_len / 1024).size(zoom * 14.0).into(),
        Slider::new(
            1..=16,
            config.llm_context_max_len as u32 / 65536,
            |n| SetProjectConfig::ContextSize(n as u64),
        ).width(zoom * 256.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0)).style(move |_| set_round_bg(gray(0.15), zoom)).padding(zoom * 8.0).into());

    Column::from_vec(panels).align_x(Horizontal::Center).spacing(zoom * 8.0).into()
}
