use super::{gray, set_round_bg};
use crate::{
    Config,
    Model,
    PermissionConfig,
    Thinking,
    ToolKind,
    ToolPermissionKind,
    list_binaries,
    truncate_chars,
};
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
    ImageEditAgent(Model),
    AgentQuestionable(Questionable),
    ContextSize(u64),
    ToggleTool(ToolKind, bool),
    ToggleSkill(String, bool),
    SetToolPermission(ToolPermissionKind, PermissionConfig),
    SetRunPermission(String, PermissionConfig),
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
        SetProjectConfig::ImageEditAgent(m) => {
            config.agents.image_edit = m;

            if m == Model::Disabled {
                set_project_config(config, SetProjectConfig::ToggleTool(ToolKind::ImageEdit, false));
            } else {
                set_project_config(config, SetProjectConfig::ToggleTool(ToolKind::ImageEdit, true));
            }
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
        SetProjectConfig::ToggleSkill(skill, enable) => {
            config.skills.get_mut(&skill).unwrap().enabled = enable;
        },
        SetProjectConfig::SetToolPermission(k, p) => {
            config.tool_permissions.insert(k, p);
        },
        SetProjectConfig::SetRunPermission(bin, p) => {
            config.run_permissions.insert(bin, p);
        },
        SetProjectConfig::OpenaiEtc1BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc1_base_url = None;
            } else {
                config.etc_models.openai_etc1_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc1Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc1_model = None;
            } else {
                config.etc_models.openai_etc1_model = Some(model);
            }
        },
        SetProjectConfig::OpenaiEtc2BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc2_base_url = None;
            } else {
                config.etc_models.openai_etc2_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc2Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc2_model = None;
            } else {
                config.etc_models.openai_etc2_model = Some(model);
            }
        },
        SetProjectConfig::OpenaiEtc3BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc3_base_url = None;
            } else {
                config.etc_models.openai_etc3_base_url = Some(url);
            }
        },
        SetProjectConfig::OpenaiEtc3Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc3_model = None;
            } else {
                config.etc_models.openai_etc3_model = Some(model);
            }
        },
    }
}

#[derive(Clone, Debug)]
pub enum SetChatConfig {
    Model(Model),
    Thinking(bool),
    WebSearch(bool),
    ChooseSystemPrompt(usize),
    OpenaiEtc1BaseUrl(String),
    OpenaiEtc1Model(String),
    OpenaiEtc2BaseUrl(String),
    OpenaiEtc2Model(String),
    OpenaiEtc3BaseUrl(String),
    OpenaiEtc3Model(String),
}

pub fn set_chat_config(config: &mut ChatConfig, system_prompts: &[String], set: SetChatConfig) {
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
        SetChatConfig::ChooseSystemPrompt(i) => {
            config.system_prompt = system_prompts[i].to_string();
        },
        SetChatConfig::OpenaiEtc1BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc1_base_url = None;
            } else {
                config.etc_models.openai_etc1_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc1Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc1_model = None;
            } else {
                config.etc_models.openai_etc1_model = Some(model);
            }
        },
        SetChatConfig::OpenaiEtc2BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc2_base_url = None;
            } else {
                config.etc_models.openai_etc2_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc2Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc2_model = None;
            } else {
                config.etc_models.openai_etc2_model = Some(model);
            }
        },
        SetChatConfig::OpenaiEtc3BaseUrl(url) => {
            if url.is_empty() {
                config.etc_models.openai_etc3_base_url = None;
            } else {
                config.etc_models.openai_etc3_base_url = Some(url);
            }
        },
        SetChatConfig::OpenaiEtc3Model(model) => {
            if model.is_empty() {
                config.etc_models.openai_etc3_model = None;
            } else {
                config.etc_models.openai_etc3_model = Some(model);
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
        ("Big Agent: ", |m| m.is_real() && m.is_llm(), config.agents.big, SetProjectConfig::BigAgent),
        ("Small Agent: ", |m| m.is_real() && m.is_llm(), config.agents.small, SetProjectConfig::SmallAgent),
        ("Search Agent: ", |m| m.supports_web_search() || *m == Model::Disabled, config.agents.search, SetProjectConfig::SearchAgent),
        ("Summary Agent: ", |m| m.is_real() && m.is_llm(), config.agents.summary, SetProjectConfig::SummaryAgent),
        ("Image Edit Agent: ", |m| m.is_image_edit() || *m == Model::Disabled, config.agents.image_edit, SetProjectConfig::ImageEditAgent),
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

    panels.push(panel_container(
        Column::from_vec(vec![
            text!("Tools").size(zoom * 14.0).into(),
            Column::from_vec(tool_checkboxes).spacing(zoom * 8.0).into(),
        ])
            .align_x(Horizontal::Center)
            .spacing(zoom * 8.0)
            .into(),
        zoom,
    ));

    let mut permission_radios: Vec<Element<SetProjectConfig>> = vec![];

    for kind in ToolPermissionKind::all() {
        let kind_str = kind.short_name();
        permission_radios.push(Row::from_vec(vec![
            text!("{}{kind_str}:", " ".repeat(16 - kind_str.len())).size(zoom * 14.0).into(),
            Radio::new("Allow", PermissionConfig::Allow, Some(config.tool_permissions.get(&kind).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetToolPermission(kind, p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Deny", PermissionConfig::Deny, Some(config.tool_permissions.get(&kind).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetToolPermission(kind, p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Ask", PermissionConfig::Ask, Some(config.tool_permissions.get(&kind).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetToolPermission(kind, p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
        ]).spacing(zoom * 16.0).into());
    }

    for binary in list_binaries() {
        permission_radios.push(Row::from_vec(vec![
            text!("{}{binary}:", " ".repeat(16 - binary.len())).size(zoom * 14.0).into(),
            Radio::new("Allow", PermissionConfig::Allow, Some(config.run_permissions.get(binary).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetRunPermission(binary.to_string(), p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Deny", PermissionConfig::Deny, Some(config.run_permissions.get(binary).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetRunPermission(binary.to_string(), p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Radio::new("Ask", PermissionConfig::Ask, Some(config.run_permissions.get(binary).cloned().unwrap_or(PermissionConfig::Ask)), |p| SetProjectConfig::SetRunPermission(binary.to_string(), p))
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
        ]).spacing(zoom * 16.0).into());
    }

    panels.push(panel_container(
        Column::from_vec(vec![
            text!("Permissions").size(zoom * 14.0).into(),
            Column::from_vec(permission_radios).spacing(zoom * 8.0).into(),
        ])
            .align_x(Horizontal::Center)
            .spacing(zoom * 8.0)
            .into(),
        zoom,
    ));

    if !config.skills.is_empty() {
        let mut skills: Vec<(_, _)> = config.skills.iter().collect();
        skills.sort_by_key(|(name, _)| name.to_string());
        let skill_checkboxes: Vec<Element<SetProjectConfig>> = skills.into_iter().map(
            move |(name, skill)| {
                Checkbox::new(skill.enabled)
                    .label(name.to_string())
                    .on_toggle(move |t| SetProjectConfig::ToggleSkill(name.to_string(), t))
                    .size(zoom * 14.0)
                    .text_size(zoom * 14.0)
                    .into()
            }
        ).collect();

        panels.push(panel_container(
            Column::from_vec(vec![
                text!("Skills").size(zoom * 14.0).into(),
                Column::from_vec(skill_checkboxes).spacing(zoom * 8.0).into(),
            ])
                .align_x(Horizontal::Center)
                .spacing(zoom * 8.0)
                .into(),
            zoom,
        ));
    }

    panels.push(panel_container(openai_etc_config(
        "openai-etc-1",
        &config.etc_models.openai_etc1_base_url,
        &config.etc_models.openai_etc1_model,
        SetProjectConfig::OpenaiEtc1BaseUrl,
        SetProjectConfig::OpenaiEtc1Model,
        zoom,
    ), zoom));
    panels.push(panel_container(openai_etc_config(
        "openai-etc-2",
        &config.etc_models.openai_etc2_base_url,
        &config.etc_models.openai_etc2_model,
        SetProjectConfig::OpenaiEtc2BaseUrl,
        SetProjectConfig::OpenaiEtc2Model,
        zoom,
    ), zoom));
    panels.push(panel_container(openai_etc_config(
        "openai-etc-3",
        &config.etc_models.openai_etc3_base_url,
        &config.etc_models.openai_etc3_model,
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
                |model| model.is_real() && model.is_llm()
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
            &config.etc_models.openai_etc1_base_url,
            &config.etc_models.openai_etc1_model,
            SetChatConfig::OpenaiEtc1BaseUrl,
            SetChatConfig::OpenaiEtc1Model,
            zoom,
        ), zoom),
        panel_container(openai_etc_config(
            "openai-etc-2",
            &config.etc_models.openai_etc2_base_url,
            &config.etc_models.openai_etc2_model,
            SetChatConfig::OpenaiEtc2BaseUrl,
            SetChatConfig::OpenaiEtc2Model,
            zoom,
        ), zoom),
        panel_container(openai_etc_config(
            "openai-etc-3",
            &config.etc_models.openai_etc3_base_url,
            &config.etc_models.openai_etc3_model,
            SetChatConfig::OpenaiEtc3BaseUrl,
            SetChatConfig::OpenaiEtc3Model,
            zoom,
        ), zoom),
    ]).spacing(zoom * 8.0).into()
}

pub fn chat_config_ui3<'c>(config: &'c ChatConfig, system_prompts: &[String], zoom: f32) -> Element<'c, SetChatConfig> {
    let mut column: Vec<Element<SetChatConfig>> = vec![
        text!("System prompt").size(zoom * 14.0).into(),
    ];

    for (i, system_prompt) in system_prompts.iter().enumerate() {
        let selected = if system_prompt == &config.system_prompt { Some(i) } else { None };

        column.push(Row::from_vec(vec![
            Radio::new("", i, selected, SetChatConfig::ChooseSystemPrompt)
                .spacing(zoom * 8.0)
                .text_size(zoom * 14.0)
                .size(zoom * 14.0)
                .into(),
            Container::new(text!("{}", truncate_chars(&system_prompt.replace("\n", "\\n"), 256)).size(zoom * 14.0))
                .width(zoom * 400.0)
                .height(zoom * 80.0)
                .padding(zoom * 8.0)
                .style(move |_| set_round_bg(gray(0.2), zoom))
                .into(),
        ]).align_y(Vertical::Center).spacing(zoom * 8.0).into());
    }

    Column::from_vec(column).spacing(zoom * 8.0).into()
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
