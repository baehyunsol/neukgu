use super::{gray, set_round_bg};
use crate::{Config, Model, Thinking, ToolKind};
use crate::chat::Config as ChatConfig;
use iced::Element;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Checkbox, Column, Container, PickList, Radio, Row, Slider, TextInput, text};

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
    ToggleTool(ToolKind, bool),
    OpenaiEtc1BaseUrl(String),
    OpenaiEtc1Model(String),
    OpenaiEtc2BaseUrl(String),
    OpenaiEtc2Model(String),
    OpenaiEtc3BaseUrl(String),
    OpenaiEtc3Model(String),
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
        SetProjectConfig::ToggleTool(tool, activate) => {
            if activate {
                config.activated_tools.push(tool);
            } else {
                config.activated_tools = config.activated_tools.iter().filter(
                    |t| **t != tool
                ).map(
                    |t| *t
                ).collect();
            }

            // sort and dedup
            config.activated_tools = ToolKind::all().into_iter().filter(
                |tool| config.activated_tools.contains(tool)
            ).collect();
        },
        SetProjectConfig::OpenaiEtc1BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc1_base_url = None;
            } else {
                config.openai_etc1_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc1Model(model) => {
            if model.is_empty() {
                config.openai_etc1_model = None;
            } else {
                config.openai_etc1_model = Some(model);
            }
        },
        SetProjectConfig::OpenaiEtc2BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc2_base_url = None;
            } else {
                config.openai_etc2_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc2Model(model) => {
            if model.is_empty() {
                config.openai_etc2_model = None;
            } else {
                config.openai_etc2_model = Some(model);
            }
        },
        SetProjectConfig::OpenaiEtc3BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc3_base_url = None;
            } else {
                config.openai_etc3_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc3Model(model) => {
            if model.is_empty() {
                config.openai_etc3_model = None;
            } else {
                config.openai_etc3_model = Some(model);
            }
        },
    }
}

#[derive(Clone, Debug)]
pub enum SetChatConfig {
    Model(Model),
    Thinking(bool),
    WebSearch(bool),
    OpenaiEtc1BaseUrl(String),
    OpenaiEtc1Model(String),
    OpenaiEtc2BaseUrl(String),
    OpenaiEtc2Model(String),
    OpenaiEtc3BaseUrl(String),
    OpenaiEtc3Model(String),
}

pub fn set_chat_config(config: &mut ChatConfig, set: SetChatConfig) {
    match set {
        SetChatConfig::Model(model) => {
            config.model = model;

            if !config.model.supports_web_search() {
                config.enable_web_search = false;
            }
        },
        SetChatConfig::Thinking(t) => {
            if t {
                config.thinking = Thinking::Enabled;
            } else {
                config.thinking = Thinking::Disabled;
            }
        },
        SetChatConfig::WebSearch(w) => {
            config.enable_web_search = w;
        },
        SetChatConfig::OpenaiEtc1BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc1_base_url = None;
            } else {
                config.openai_etc1_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc1Model(model) => {
            if model.is_empty() {
                config.openai_etc1_model = None;
            } else {
                config.openai_etc1_model = Some(model);
            }
        },
        SetChatConfig::OpenaiEtc2BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc2_base_url = None;
            } else {
                config.openai_etc2_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc2Model(model) => {
            if model.is_empty() {
                config.openai_etc2_model = None;
            } else {
                config.openai_etc2_model = Some(model);
            }
        },
        SetChatConfig::OpenaiEtc3BaseUrl(url) => {
            if url.is_empty() {
                config.openai_etc3_base_url = None;
            } else {
                config.openai_etc3_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc3Model(model) => {
            if model.is_empty() {
                config.openai_etc3_model = None;
            } else {
                config.openai_etc3_model = Some(model);
            }
        },
    }
}

pub fn panel_container<'m, Message: 'm>(panel: Element<'m, Message>, zoom: f32) -> Element<'m, Message> {
    Container::new(panel).style(move |_| set_round_bg(gray(0.15), zoom)).padding(zoom * 8.0).into()
}

pub fn config_ui<'c>(config: &'c Config, zoom: f32) -> Element<'c, SetProjectConfig> {

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

        panels.push(panel_container(Row::from_vec(vec![text!("{title}").size(zoom * 14.0).into(), radios]).align_y(Vertical::Center).into(), zoom));
    }

    let curr_q = match config.user_response_timeout {
        0 => Questionable::Never,
        999_999 => Questionable::Always,
        _ => Questionable::Maybe,
    };
    panels.push(panel_container(Column::from_vec(vec![
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
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0).into(), zoom));

    panels.push(panel_container(Column::from_vec(vec![
        text!("Context size: {} KiB", config.llm_context_max_len / 1024).size(zoom * 14.0).into(),
        Slider::new(
            1..=16,
            config.llm_context_max_len as u32 / 65536,
            |n| SetProjectConfig::ContextSize(n as u64),
        ).width(zoom * 384.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0).into(), zoom));

    let mut tool_checkboxes = vec![];

    for tools in ToolKind::all().chunks(4) {
        let tools: Vec<ToolKind> = tools.to_vec();
        tool_checkboxes.push(Row::from_vec(tools.into_iter().map(
            move |tool| Checkbox::new(config.activated_tools.contains(&tool))
                .label(tool.tag_name())
                .on_toggle_maybe(
                    if tool.optional() {
                        Some(move |t| SetProjectConfig::ToggleTool(tool, t))
                    } else {
                        None
                    }
                )
                .size(zoom * 14.0)
                .text_size(zoom * 14.0)
                .into()
        ).collect()).spacing(zoom * 8.0).into());
    }

    panels.push(panel_container(Column::from_vec(vec![
        text!("Tools").size(zoom * 14.0).into(),
        Column::from_vec(tool_checkboxes).spacing(zoom * 8.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0).into(), zoom));

    panels.push(panel_container(openai_etc_config(
        "openai-etc-1",
        &config.openai_etc1_base_url,
        &config.openai_etc1_model,
        SetProjectConfig::OpenaiEtc1BaseUrl,
        SetProjectConfig::OpenaiEtc1Model,
        zoom,
    ), zoom));
    panels.push(panel_container(openai_etc_config(
        "openai-etc-2",
        &config.openai_etc2_base_url,
        &config.openai_etc2_model,
        SetProjectConfig::OpenaiEtc2BaseUrl,
        SetProjectConfig::OpenaiEtc2Model,
        zoom,
    ), zoom));
    panels.push(panel_container(openai_etc_config(
        "openai-etc-3",
        &config.openai_etc3_base_url,
        &config.openai_etc3_model,
        SetProjectConfig::OpenaiEtc3BaseUrl,
        SetProjectConfig::OpenaiEtc3Model,
        zoom,
    ), zoom));

    Column::from_vec(panels).align_x(Horizontal::Center).spacing(zoom * 8.0).into()
}

pub fn chat_config_ui1<'c>(config: &'c ChatConfig, zoom: f32) -> Element<'c, SetChatConfig> {
    Row::from_vec(vec![
        text!("Model:").size(zoom * 14.0).into(),
        PickList::new(
            Model::all().into_iter().filter(
                |model| *model != Model::Mock && *model != Model::Disabled
            ).collect::<Vec<_>>(),
            Some(config.model),
            |model| SetChatConfig::Model(model),
        )
            .text_size(zoom * 14.0)
            .width(zoom * 160.0)
            .into(),
        Checkbox::new(config.thinking != Thinking::Disabled)
            .label("Thinking")
            .on_toggle(|t| SetChatConfig::Thinking(t))
            .size(zoom * 14.0)
            .text_size(zoom * 14.0)
            .into(),
        Checkbox::new(config.enable_web_search)
            .label("Web Search")
            .on_toggle_maybe(if config.model.supports_web_search() {
                Some(|s| SetChatConfig::WebSearch(s))
            } else {
                None
            })
            .size(zoom * 14.0)
            .text_size(zoom * 14.0)
            .into(),
    ])
        .spacing(zoom * 8.0)
        .height(zoom * 48.0)
        .align_y(Vertical::Center)
        .into()
}

pub fn chat_config_ui2<'c>(config: &'c ChatConfig, zoom: f32) -> Element<'c, SetChatConfig> {
    Row::from_vec(vec![
        panel_container(openai_etc_config(
            "openai-etc-1",
            &config.openai_etc1_base_url,
            &config.openai_etc1_model,
            SetChatConfig::OpenaiEtc1BaseUrl,
            SetChatConfig::OpenaiEtc1Model,
            zoom,
        ), zoom),
        panel_container(openai_etc_config(
            "openai-etc-2",
            &config.openai_etc2_base_url,
            &config.openai_etc2_model,
            SetChatConfig::OpenaiEtc2BaseUrl,
            SetChatConfig::OpenaiEtc2Model,
            zoom,
        ), zoom),
        panel_container(openai_etc_config(
            "openai-etc-3",
            &config.openai_etc3_base_url,
            &config.openai_etc3_model,
            SetChatConfig::OpenaiEtc3BaseUrl,
            SetChatConfig::OpenaiEtc3Model,
            zoom,
        ), zoom),
    ]).spacing(zoom * 8.0).into()
}

fn openai_etc_config<'c, Message: Clone + 'c, F1: Fn(String) -> Message + 'c, F2: Fn(String) -> Message + 'c>(
    title: &'static str,
    base_url: &'c Option<String>,
    model_name: &'c Option<String>,
    set_base_url: F1,
    set_model: F2,
    zoom: f32,
) -> Element<'c, Message> {
    Column::from_vec(vec![
        text!("{title}").size(zoom * 14.0).into(),
        Row::from_vec(vec![
            text!("base url:").size(zoom * 14.0).into(),
            TextInput::new("", base_url.as_ref().map_or("", |s| s))
                .size(zoom * 14.0)
                .on_input(set_base_url)
                .width(zoom * 320.0)
                .into(),
        ]).align_y(Vertical::Center).spacing(zoom * 8.0).into(),
        Row::from_vec(vec![
            text!("   model:").size(zoom * 14.0).into(),
            TextInput::new("", model_name.as_ref().map_or("", |s| s))
                .size(zoom * 14.0)
                .on_input(set_model)
                .width(zoom * 320.0)
                .into(),
        ]).align_y(Vertical::Center).spacing(zoom * 8.0).into(),
    ]).align_x(Horizontal::Center).spacing(zoom * 8.0).into()
}
