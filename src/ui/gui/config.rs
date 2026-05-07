use crate::{Config, Model};
use iced::Element;
use iced::widget::{Column, Radio, Row, text};

#[derive(Clone, Debug)]
pub enum SetProjectConfig {
    SetBigAgent(Model),
    SetSmallAgent(Model),
    SetSearchAgent(Model),
    SetSummaryAgent(Model),
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
    }
}

pub fn config_ui<'c, 'm>(config: &'c Config, zoom: f32) -> Element<'m, SetProjectConfig> {
    let big_agent = Row::from_vec(vec![
        text!("    Big Agent: ").size(zoom * 14.0).into(),
        Row::from_vec(Model::all().into_iter().filter(
            |m| *m != Model::Mock && *m != Model::Disabled
        ).map(
            |m| Radio::new(
                m.short_name(),
                m,
                Some(config.agents.big),
                |m| SetProjectConfig::SetBigAgent(m),
            ).into()
        ).collect()).spacing(zoom * 8.0).into(),
    ]);

    let small_agent = Row::from_vec(vec![
        text!("  Small Agent: ").size(zoom * 14.0).into(),
        Row::from_vec(Model::all().into_iter().filter(
            |m| *m != Model::Mock && *m != Model::Disabled
        ).map(
            |m| Radio::new(
                m.short_name(),
                m,
                Some(config.agents.small),
                |m| SetProjectConfig::SetSmallAgent(m),
            ).into()
        ).collect()).spacing(zoom * 8.0).into(),
    ]);

    let search_agent = Row::from_vec(vec![
        text!(" Search Agent: ").size(zoom * 14.0).into(),
        Row::from_vec(Model::all().into_iter().filter(
            |m| *m != Model::Mock
        ).map(
            |m| Radio::new(
                m.short_name(),
                m,
                Some(config.agents.search),
                |m| SetProjectConfig::SetSearchAgent(m),
            ).into()
        ).collect()).spacing(zoom * 8.0).into(),
    ]);

    let summary_agent = Row::from_vec(vec![
        text!("Summary Agent: ").size(zoom * 14.0).into(),
        Row::from_vec(Model::all().into_iter().filter(
            |m| *m != Model::Mock
        ).map(
            |m| Radio::new(
                m.short_name(),
                m,
                Some(config.agents.summary),
                |m| SetProjectConfig::SetSummaryAgent(m),
            ).into()
        ).collect()).spacing(zoom * 8.0).into(),
    ]);

    Column::from_vec(vec![
        big_agent.into(),
        small_agent.into(),
        search_agent.into(),
        summary_agent.into(),
    ]).into()
}
