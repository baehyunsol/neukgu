use super::{button, gray, green, white};
use super::popup::{PopupContext, PopupMessage, into_popup};
use iced::{Background, Element, Length, Task};
use iced::alignment::Horizontal;
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, Scrollable, TextInput, text};
use iced::widget::container::{Container, Style};
use iced::widget::operation::focus;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub missing_api_keys: Vec<(String, Vec<String>)>,
    pub text_input_ids: Vec<Id>,
    pub key1: String,
    pub key2: String,
    pub key3: String,
    pub key4: String,
}

impl IcedContext {
    pub fn new(missing_api_keys: Vec<(String, Vec<String>)>) -> IcedContext {
        IcedContext {
            missing_api_keys,
            text_input_ids: (0..4).map(|_| Id::unique()).collect(),
            key1: String::new(),
            key2: String::new(),
            key3: String::new(),
            key4: String::new(),
        }
    }

    pub fn focus(&self) -> Task<IcedMessage> {
        if let Some(id) = self.text_input_ids.get(0) && self.key1.is_empty() {
            focus(id.clone())
        }

        else if let Some(id) = self.text_input_ids.get(1) && self.key2.is_empty() {
            focus(id.clone())
        }

        else if let Some(id) = self.text_input_ids.get(2) && self.key3.is_empty() {
            focus(id.clone())
        }

        else if let Some(id) = self.text_input_ids.get(3) && self.key4.is_empty() {
            focus(id.clone())
        }

        else {
            Task::none()
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    EditKey1(String),
    EditKey2(String),
    EditKey3(String),
    EditKey4(String),
    Enter,
    Focus(Id),
}

// You can't close/back/copy this popup.
impl PopupMessage for IcedMessage {
    fn close_popup() -> Self { unreachable!() }
    fn back_popup() -> Self { unreachable!() }
    fn copy_popup_content() -> Self { unreachable!() }
    fn open_scratch_pad() -> Self { unreachable!() }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::EditKey1(key) => {
            context.key1 = key;
        },
        IcedMessage::EditKey2(key) => {
            context.key2 = key;
        },
        IcedMessage::EditKey3(key) => {
            context.key3 = key;
        },
        IcedMessage::EditKey4(key) => {
            context.key4 = key;
        },
        IcedMessage::Enter => {},
        IcedMessage::Focus(id) => {
            return focus(id);
        },
    }

    Task::none()
}

pub fn get_api_keys_popup<'ac, 'pc, Context: PopupContext>(
    api_keys_context: &'ac IcedContext,
    popup_context: &'pc Context,
    zoom: f32,
) -> Element<'ac, IcedMessage> {
    fn input<'e, 'm, F: Fn(String) -> IcedMessage + 'm>(
        env_var: &'e str,
        agent_names: &'e [String],
        id: Id,
        value: &'m str,
        on_input: F,
        on_submit: IcedMessage,
        zoom: f32,
    ) -> Element<'m, IcedMessage> {
        let agent_names = if agent_names.is_empty() {
            String::new()
        } else {
            format!(" ({})", agent_names.iter().map(|agent| format!("{agent}-agent")).collect::<Vec<_>>().join(", "))
        };

        Container::new(
            Column::from_vec(vec![
                text!("{env_var}{agent_names}").color(white()).size(zoom * 14.0).into(),
                TextInput::new("", value)
                    .id(id)
                    .size(zoom * 14.0)
                    .width(zoom * 320.0)
                    .on_input(on_input)
                    .on_submit(on_submit)
                    .into(),
            ])
                .align_x(Horizontal::Center)
                .spacing(zoom * 8.0)
        ).style(move |_| Style {
            background: Some(Background::Color(gray(0.3))),
            border: Border {
                color: white(),
                width: 0.0,
                radius: Radius::new(zoom * 8.0),
            },
            ..Style::default()
        }).padding(zoom * 20.0).into()
    }

    let mut column: Vec<Element<IcedMessage>> = vec![text!("Enter API Keys").color(white()).size(zoom * 18.0).into()];
    column.push(input(
        &api_keys_context.missing_api_keys[0].0,
        &api_keys_context.missing_api_keys[0].1,
        api_keys_context.text_input_ids[0].clone(),
        &api_keys_context.key1,
        IcedMessage::EditKey1,
        if api_keys_context.missing_api_keys.len() == 1 {
            IcedMessage::Enter
        } else {
            IcedMessage::Focus(api_keys_context.text_input_ids[1].clone())
        },
        zoom,
    ));

    if let Some((env_var, agent_names)) = api_keys_context.missing_api_keys.get(1) {
        column.push(input(
            env_var,
            agent_names,
            api_keys_context.text_input_ids[1].clone(),
            &api_keys_context.key2,
            IcedMessage::EditKey2,
            if api_keys_context.missing_api_keys.len() == 2 {
                IcedMessage::Enter
            } else {
                IcedMessage::Focus(api_keys_context.text_input_ids[2].clone())
            },
            zoom,
        ));
    }

    if let Some((env_var, agent_names)) = api_keys_context.missing_api_keys.get(2) {
        column.push(input(
            env_var,
            agent_names,
            api_keys_context.text_input_ids[2].clone(),
            &api_keys_context.key3,
            IcedMessage::EditKey3,
            if api_keys_context.missing_api_keys.len() == 3 {
                IcedMessage::Enter
            } else {
                IcedMessage::Focus(api_keys_context.text_input_ids[3].clone())
            },
            zoom,
        ));
    }

    if let Some((env_var, agent_names)) = api_keys_context.missing_api_keys.get(3) {
        column.push(input(
            env_var,
            agent_names,
            api_keys_context.text_input_ids[3].clone(),
            &api_keys_context.key4,
            IcedMessage::EditKey4,
            IcedMessage::Enter,
            zoom,
        ));
    }

    column.push(button("Enter", IcedMessage::Enter, green(), zoom).into());

    into_popup(
        Scrollable::new(
            Column::from_vec(column)
                .align_x(Horizontal::Center)
                .width(Length::Fill)
                .spacing(zoom * 20.0)
        )
            .into(),
        popup_context,
    )
}
