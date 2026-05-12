use super::{gray, white};
use crate::{Config, Model};
use iced::{Background, Element};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::widget::{Column, Radio, Row, text};
use iced::widget::container::{Container, Style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Questionable {
    Always,
    Maybe,
    Never,
}

#[derive(Clone, Debug)]
pub enum SetProjectConfig {
    SetBigAgent(Model),
    SetSmallAgent(Model),
    SetSearchAgent(Model),
    SetSummaryAgent(Model),
    SetAgentQuestionable(Questionable),
}

pub fn set_project_config(config: &mut Config, set: SetProjectConfig) {
    match set {
        SetProjectConfig::SetBigAgent(m) => {
            config.agents.big = m;
        },
        SetProjectConfig::SetSmallAgent(m) => {
            config.agents.small = m;
        },
        SetProjectConfig::SetSearchAgent(m) => {
            config.agents.search = m;
        },
        SetProjectConfig::SetSummaryAgent(m) => {
            config.agents.summary = m;
        },
        SetProjectConfig::SetAgentQuestionable(q) => {
            let timeout = match q {
                Questionable::Never => 0,
                Questionable::Always => 999_999,
                Questionable::Maybe => 300,
            };

            config.user_response_timeout = timeout;
        },
    }
}

pub fn config_ui<'c, 'm>(config: &'c Config, zoom: f32) -> Element<'m, SetProjectConfig> {
    let mut panels = vec![];
    let agent_panels: Vec<(&str, fn(&Model) -> bool, Model, fn(Model) -> SetProjectConfig)> = vec![
        ("    Big Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.big, SetProjectConfig::SetBigAgent),
        ("  Small Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.small, SetProjectConfig::SetSmallAgent),
        (" Search Agent: ", |m| m.supports_web_search(), config.agents.search, SetProjectConfig::SetSearchAgent),
        ("Summary Agent: ", |m| *m != Model::Mock && *m != Model::Disabled, config.agents.summary, SetProjectConfig::SetSummaryAgent),
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
        ).style(move |_| panel_style(zoom)).padding(zoom * 8.0).into());
    }

    let curr_q = match config.user_response_timeout {
        0 => Questionable::Never,
        999_999 => Questionable::Always,
        _ => Questionable::Maybe,
    };
    panels.push(Container::new(Column::from_vec(vec![
        text!("Will you answer neukgu's questions?").size(zoom * 14.0).into(),
        Row::from_vec(vec![
            Radio::new("Always", Questionable::Always, Some(curr_q), SetProjectConfig::SetAgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("If I'm available", Questionable::Maybe, Some(curr_q), SetProjectConfig::SetAgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Never", Questionable::Never, Some(curr_q), SetProjectConfig::SetAgentQuestionable)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
        ]).spacing(zoom * 16.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0)).style(move |_| panel_style(zoom)).padding(zoom * 8.0).into());

    Column::from_vec(panels).align_x(Horizontal::Center).spacing(zoom * 8.0).into()
}

fn panel_style(zoom: f32) -> Style {
    Style {
        background: Some(Background::Color(gray(0.15))),
        border: Border {
            color: white(),
            width: 0.0,
            radius: Radius::new(zoom * 8.0),
        },
        ..Style::default()
    }
}
