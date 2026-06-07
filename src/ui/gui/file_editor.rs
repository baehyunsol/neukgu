use super::{blue, button, gray, set_round_bg, yellow};
use super::file_change::render_udiff;
use crate::{Error, patch_diff, revert_hunks};
use iced::{Element, Size, Task};
use iced::keyboard::Key;
use iced::widget::{Column, Container, Id, Row, Scrollable, text};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Binding,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    KeyPress,
    TextEditor,
};
use ragit_fs::{WriteMode, extension, read_string, write_string};
use similar::Algorithm as DiffAlgorithm;
use similar::udiff::unified_diff;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub path: String,
    pub window_size: Size,
    pub history_view: bool,
    pub last_saved_content: String,
    pub diffs: Vec<String>,
    pub text_editor_content: TextEditorContent,
    pub text_editor_id: Id,
    pub popup_scroll_id: Id,
    pub syntax_highlight: String,
    pub zoom: f32,
}

impl IcedContext {
    pub fn new(
        path: &str,
        popup_scroll_id: Id,
        window_size: Size,
        zoom: f32,
    ) -> Result<IcedContext, Error> {
        let mut result = IcedContext {
            path: path.to_string(),
            window_size,
            history_view: false,
            last_saved_content: String::new(),
            diffs: vec![],
            text_editor_content: TextEditorContent::new(),
            text_editor_id: Id::unique(),
            popup_scroll_id,
            syntax_highlight: extension(path)?.unwrap_or(String::from("txt")),
            zoom,
        };
        let content = read_string(path)?;
        result.last_saved_content = content.to_string();
        result.set_text_editor_content(content);
        Ok(result)
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.text_editor_content.perform(TextEditorAction::SelectAll);
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    EditText(TextEditorAction),
    ToggleHistoryView,
    Save,
    Rollback(usize),
    Notify(String),
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::EditText(action) => {
            context.text_editor_content.perform(action);
        },
        IcedMessage::ToggleHistoryView => {
            context.history_view = !context.history_view;
        },
        IcedMessage::Save => {
            let content = context.text_editor_content.text();

            if content == context.last_saved_content {
                return Ok(Task::done(IcedMessage::Notify(String::from("Nothing to save"))));
            }

            else {
                context.diffs.push(unified_diff(
                    DiffAlgorithm::Patience,
                    &context.last_saved_content,
                    &content,
                    5,
                    None,
                ));
                context.last_saved_content = content.to_string();
                write_string(
                    &context.path,
                    &content,
                    WriteMode::CreateOrTruncate,
                )?;
                return Ok(Task::done(IcedMessage::Notify(String::from("Saved"))));
            }
        },
        IcedMessage::Rollback(index) => {
            for (j, diff) in context.diffs.iter().enumerate().rev() {
                if j > index {
                    let hunks = revert_hunks(&diff);

                    for hunk in hunks.iter() {
                        context.last_saved_content = patch_diff(&context.last_saved_content, hunk).unwrap();
                    }
                }

                else {
                    break;
                }
            }

            context.set_text_editor_content(context.last_saved_content.to_string());
            context.diffs = context.diffs[(index + 1)..].to_vec();
            write_string(
                &context.path,
                &context.last_saved_content,
                WriteMode::CreateOrTruncate,
            )?;
            return Ok(Task::done(IcedMessage::ToggleHistoryView));
        },
        IcedMessage::Notify(_) => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    if context.history_view {
        if context.diffs.is_empty() {
            Column::from_vec(vec![
                text!("No Changes").size(context.zoom * 14.0).into(),
                button("Back", IcedMessage::ToggleHistoryView, yellow(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).into()
        }

        else {
            let mut column: Vec<Element<IcedMessage>> = context.diffs.iter().enumerate().rev().map(
                |(i, diff)| Container::new(
                    Column::from_vec(vec![
                        button("Rollback", IcedMessage::Rollback(i), blue(), context.zoom).into(),
                        render_udiff(diff, context.window_size.width, context.zoom),
                    ]).spacing(context.zoom * 4.0)
                ).padding(context.zoom * 6.0).style(move |_| set_round_bg(gray(0.2), context.zoom)).into()
            ).collect();
            column.push(button("Back", IcedMessage::ToggleHistoryView, yellow(), context.zoom).into());

            Scrollable::new(
                Column::from_vec(column)
                    .spacing(context.zoom * 12.0)
            )
                .id(context.popup_scroll_id.clone())
                .into()
        }
    }

    else {
        Column::from_vec(vec![
            text!("{}", context.path).size(context.zoom * 18.0).into(),
            TextEditor::new(&context.text_editor_content)
                .id(context.text_editor_id.clone())
                .size(context.zoom * 14.0)
                .width(context.window_size.width)
                .height(context.window_size.height * 0.5)
                .highlight(&context.syntax_highlight, iced::highlighter::Theme::SolarizedDark)
                .on_action(IcedMessage::EditText)
                .key_binding(move |key_press| {
                    let KeyPress { key, modifiers, .. } = &key_press;

                    match (key.as_ref(), modifiers.control(), modifiers.alt(), modifiers.shift()) {
                        (Key::Character("s"), true, false, false) => Some(Binding::Custom(IcedMessage::Save)),
                        (Key::Character("z"), true, false, false) => Some(Binding::Custom(IcedMessage::ToggleHistoryView)),
                        _ => Binding::from_key_press(key_press),
                    }
                })
                .into(),
            Row::from_vec(vec![
                button("Save", IcedMessage::Save, blue(), context.zoom).into(),
                button("History", IcedMessage::ToggleHistoryView, yellow(), context.zoom).into(),
            ]).spacing(context.zoom * 8.0).into(),
        ])
            .spacing(context.zoom * 8.0)
            .into()
    }
}
