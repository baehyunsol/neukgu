use super::{blue, button, green, red};
use crate::Error;
use iced::{Element, Length, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Id, Row, Scrollable, Space, TextInput, text};
use iced::widget::operation::focus;
use ragit_fs::{
    WriteMode,
    exists,
    join,
    read_bytes,
    write_bytes,
};
use serde::{Deserialize, Serialize};

mod crypt;

pub struct IcedContext {
    pub global_index_dir: String,
    pub is_locked: bool,
    pub password: Option<Vec<u8>>,
    pub password_error: bool,
    pub api_keys: Vec<ApiKey>,
    pub short_text_editor_id: Id,
    pub short_text_editor_content: String,
}

impl IcedContext {
    pub fn new(global_index_dir: String) -> IcedContext {
        IcedContext {
            global_index_dir,
            is_locked: true,
            password: None,
            password_error: false,
            api_keys: vec![],
            short_text_editor_id: Id::unique(),
            short_text_editor_content: String::new(),
        }
    }

    // Ok(Some(api_keys)) -> correct password
    // Ok(None) -> wrong password
    // Err(_) -> file IO error that has nothing to do with the password
    pub fn load_api_keys(&self, password: &[u8]) -> Result<Option<Vec<ApiKey>>, Error> {
        let api_keys_at = join(&self.global_index_dir, "api-keys")?;

        if exists(&api_keys_at) {
            let api_keys_encrypted = read_bytes(&api_keys_at)?;

            match crypt::decrypt(&api_keys_encrypted, password) {
                Ok(json) => match serde_json::from_slice(&json) {
                    Ok(api_keys) => Ok(Some(api_keys)),
                    Err(_) => Ok(None),
                },
                Err(_) => Ok(None),
            }
        }

        // init model store with this password
        else {
            Ok(Some(vec![
                ApiKey { name: String::from("OPENAI_API_KEY"), key: String::new() },
                ApiKey { name: String::from("ANTHROPIC_API_KEY"), key: String::new() },
                ApiKey { name: String::from("MOCK_API_KEY"), key: String::from("mock-1234") },
            ]))
        }
    }

    pub fn refresh(&mut self) -> Result<(), Error> {
        if !self.is_locked && let Some(password) = &self.password && let Some(api_keys) = self.load_api_keys(password)? {
            self.api_keys = api_keys;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    EnterPassword,
    EditShortTextEdit(String),
    AddApiKey,
    Save,
    SetApiKeyName(usize, String),
    SetApiKeyKey(usize, String),
    DeleteApiKey(usize),
    CopyString(String),
    Focus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiKey {
    pub name: String,
    pub key: String,
}

pub fn view<'c>(context: &'c IcedContext, scroll_id: Id, zoom: f32) -> Element<'c, IcedMessage> {
    if context.is_locked {
        Column::from_vec(vec![
            text!("Model Store").size(zoom * 18.0).into(),
            if context.password_error {
                text!("Wrong password").size(zoom * 14.0).color(red()).into()
            } else {
                Space::new().into()
            },
            Row::from_vec(vec![
                text!("password:").size(zoom * 14.0).into(),
                TextInput::new("", &context.short_text_editor_content)
                    .size(zoom * 14.0)
                    .id(context.short_text_editor_id.clone())
                    .width(zoom * 256.0)
                    .secure(true)
                    .on_input(IcedMessage::EditShortTextEdit)
                    .on_submit(IcedMessage::EnterPassword)
                    .into(),
            ]).spacing(zoom * 8.0).align_y(Vertical::Center).into(),
            button("Enter", IcedMessage::EnterPassword, green(), zoom).into(),
        ]).spacing(zoom * 8.0).width(Length::Fill).align_x(Horizontal::Center).into()
    }

    else {
        Scrollable::new(
            Column::from_vec(vec![
                text!("Model Store").size(zoom * 18.0).into(),
                text!("--- Api Keys ---").size(zoom * 14.0).into(),
                Column::from_vec(context.api_keys.iter().enumerate().map(
                    |(i, ApiKey { name, key })| Row::from_vec(vec![
                        TextInput::new("", name)
                            .size(zoom * 14.0)
                            .width(zoom * 160.0)
                            .on_input(move |name| IcedMessage::SetApiKeyName(i, name))
                            .into(),
                        text!(":").size(zoom * 14.0).into(),
                        TextInput::new("", key)
                            .size(zoom * 14.0)
                            .width(zoom * 320.0)
                            .on_input(move |name| IcedMessage::SetApiKeyKey(i, name))
                            .into(),
                        button("Copy", IcedMessage::CopyString(key.to_string()), blue(), zoom).into(),
                        button("Delete", IcedMessage::DeleteApiKey(i), red(), zoom).into(),
                    ]).spacing(zoom * 8.0).into()
                ).collect()).spacing(zoom * 8.0).into(),
                Row::from_vec(vec![
                    button("Add", IcedMessage::AddApiKey, blue(), zoom).into(),
                    button("Save", IcedMessage::Save, blue(), zoom).into(),
                ]).spacing(zoom * 8.0).into(),
            ])
                .spacing(zoom * 8.0)
                .width(Length::Fill)
                .align_x(Horizontal::Center)
        ).id(scroll_id).into()
    }
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::EnterPassword => {
            let password = context.short_text_editor_content.as_bytes().to_vec();

            match context.load_api_keys(&password)? {
                Some(api_keys) => {
                    context.password = Some(password);
                    context.api_keys = api_keys;
                    context.is_locked = false;
                },
                None => {
                    context.password_error = true;
                },
            }

            context.short_text_editor_content = String::new();
        },
        IcedMessage::EditShortTextEdit(s) => {
            context.short_text_editor_content = s;
        },
        IcedMessage::AddApiKey => {
            context.api_keys.push(ApiKey { name: String::new(), key: String::new() });
        },
        IcedMessage::Save => {
            let api_keys_at = join(&context.global_index_dir, "api-keys")?;
            let data = serde_json::to_vec_pretty(&context.api_keys).unwrap();
            let api_keys_encrypted = crypt::encrypt(&data, context.password.as_ref().unwrap());
            write_bytes(&api_keys_at, &api_keys_encrypted, WriteMode::CreateOrTruncate)?;
        },
        IcedMessage::SetApiKeyName(i, name) => {
            context.api_keys[i].name = name;
        },
        IcedMessage::SetApiKeyKey(i, key) => {
            context.api_keys[i].key = key;
        },
        IcedMessage::DeleteApiKey(i) => {
            context.api_keys.remove(i);
        },
        IcedMessage::CopyString(s) => {
            return Ok(iced::clipboard::write(s.to_string()));
        },
        IcedMessage::Focus => {
            return Ok(focus(context.short_text_editor_id.clone()));
        },
    }

    Ok(Task::none())
}
