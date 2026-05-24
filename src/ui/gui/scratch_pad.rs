use super::{black, blue, button, disabled_button, gray, red, white};
use iced::{Background, Element, Size, Task};
use iced::alignment::Horizontal;
use iced::border::{Border, Radius};
use iced::keyboard::{Key, Modifiers, key::Named as NamedKey};
use iced::widget::{Column, Id, Row, Scrollable, Space, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{
    Handle as ImageHandle,
    Viewer as ImageViewer,
};
use iced::widget::operation::{RelativeOffset, snap_to};
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    Style as TextEditorStyle,
    TextEditor,
};
use std::sync::Arc;

pub struct IcedContext {
    pub window_size: Size,
    pub alignment: Horizontal,
    pub title: Option<String>,
    pub content: Content,
    pub is_expanded: bool,
    pub popup_scroll_id: Id,
    pub zoom: f32,
    pub text_editor_content: TextEditorContent,
}

impl IcedContext {
    pub fn new(
        title: Option<String>,
        content: Content,
        window_size: Size,
    ) -> IcedContext {
        let mut context = IcedContext {
            window_size,
            alignment: Horizontal::Right,
            title,
            content: content.clone(),
            is_expanded: true,
            popup_scroll_id: Id::unique(),
            zoom: 1.0,
            text_editor_content: TextEditorContent::new(),
        };

        if let Content::Text { content, .. } = content {
            context.set_text_editor_content(content);
        }

        context
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.text_editor_content.perform(TextEditorAction::SelectAll);
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    ToggleExpand,
    SetAlignment(Horizontal),
    ZoomIn,
    ZoomOut,
    Close,
}

#[derive(Clone, Debug)]
pub enum Content {
    Image { path: String },
    Text {
        content: String,
        extension: Option<String>,
    },
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::KeyPressed { key, modifiers } => match (key.as_ref(), modifiers.alt()) {
            (Key::Named(NamedKey::Escape), false) => {
                return Task::done(IcedMessage::Close);
            },
            (Key::Named(NamedKey::ArrowUp), false) => {
                return snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 0.0 });
            },
            (Key::Named(NamedKey::ArrowDown), false) => {
                return snap_to(context.popup_scroll_id.clone(), RelativeOffset { x: 0.0, y: 1.0 });
            },
            (Key::Named(NamedKey::ArrowLeft), false) => match context.alignment {
                Horizontal::Left => {},
                Horizontal::Center => {
                    return Task::done(IcedMessage::SetAlignment(Horizontal::Left));
                },
                Horizontal::Right => {
                    return Task::done(IcedMessage::SetAlignment(Horizontal::Center));
                },
            },
            (Key::Named(NamedKey::ArrowRight), false) => match context.alignment {
                Horizontal::Left => {
                    return Task::done(IcedMessage::SetAlignment(Horizontal::Center));
                },
                Horizontal::Center => {
                    return Task::done(IcedMessage::SetAlignment(Horizontal::Right));
                },
                Horizontal::Right => {},
            },
            (Key::Character("e"), false) => {
                return Task::done(IcedMessage::ToggleExpand);
            },
            (Key::Character("-"), false) => {
                return Task::done(IcedMessage::ZoomOut);
            },
            (Key::Character("="), false) => {
                return Task::done(IcedMessage::ZoomIn);
            },
            _ => {},
        },
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::ToggleExpand => {
            context.is_expanded = !context.is_expanded;
        },
        IcedMessage::SetAlignment(a) => {
            context.alignment = a;
        },
        IcedMessage::ZoomIn => {
            context.zoom = context.zoom.min(2.4) + 0.1;
        },
        IcedMessage::ZoomOut => {
            context.zoom = context.zoom.max(0.2) - 0.1;
        },
        IcedMessage::Close => unreachable!(),
    }

    Task::none()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let x = match context.alignment {
        Horizontal::Left => context.window_size.width * 0.05,
        Horizontal::Center => context.window_size.width * 0.3,
        Horizontal::Right => context.window_size.width * 0.55,
    };
    let y = context.window_size.height * 0.15;
    let w = if context.is_expanded {
        context.window_size.width * 0.4
    } else {
        context.window_size.width * 0.2
    };
    let h = if context.is_expanded {
        context.window_size.height * 0.65
    } else {
        context.window_size.height * 0.15
    };

    Column::from_vec(vec![
        Space::new().height(y).into(),
        Row::from_vec(vec![
            Space::new().width(x).into(),
            render_scratch_pad(context, w, h),
        ]).into(),
    ]).into()
}

fn render_scratch_pad<'c>(context: &'c IcedContext, w: f32, h: f32) -> Element<'c, IcedMessage> {
    let mut content: Element<IcedMessage> = match &context.content {
        Content::Image { path } => ImageViewer::new(ImageHandle::from_path(path)).into(),
        Content::Text { content: _, extension } => TextEditor::new(&context.text_editor_content).width(context.window_size.width).size(context.zoom * 14.0).highlight(
            &if let Some(extension) = extension { extension.to_string() } else { String::from("txt") },
            iced::highlighter::Theme::InspiredGitHub,
        ).style(|_, _| TextEditorStyle {
            background: Background::Color(gray(0.85)),
            border: Border::default(),
            placeholder: gray(0.15),
            value: black(),
            selection: gray(0.4),
        }).into(),
    };

    if let Some(title) = &context.title {
        content = Column::from_vec(vec![
            text!("{title}").color(black()).size(context.zoom * 18.0).into(),
            content,
        ]).spacing(context.zoom * 8.0).into();
    }

    Container::new(
        Column::from_vec(vec![
            render_top_buttons(context),
            Scrollable::new(Container::new(content).padding(context.zoom * 8.0)).id(context.popup_scroll_id.clone()).into(),
        ]).spacing(context.zoom * 8.0)
    ).style(|_| Style {
        background: Some(Background::Color(white())),
        border: Border {
            color: black(),
            width: context.zoom * 4.0,
            radius: Radius::new(context.zoom * 8.0),
        },
        ..Style::default()
    }).padding(context.zoom * 6.0).width(w).height(h).into()
}

fn render_top_buttons<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let mut buttons = vec![
        button("X", IcedMessage::Close, red(), context.zoom).into(),
        if context.is_expanded {
            button("▼", IcedMessage::ToggleExpand, black(), context.zoom).into()
        } else {
            button("▶", IcedMessage::ToggleExpand, black(), context.zoom).into()
        },
    ];

    match context.alignment {
        Horizontal::Left => {
            buttons.push(disabled_button("<<", blue(), context.zoom).into());
            buttons.push(button(">>", IcedMessage::SetAlignment(Horizontal::Center), blue(), context.zoom).into());
        },
        Horizontal::Center => {
            buttons.push(button("<<", IcedMessage::SetAlignment(Horizontal::Left), blue(), context.zoom).into());
            buttons.push(button(">>", IcedMessage::SetAlignment(Horizontal::Right), blue(), context.zoom).into());
        },
        Horizontal::Right => {
            buttons.push(button("<<", IcedMessage::SetAlignment(Horizontal::Center), blue(), context.zoom).into());
            buttons.push(disabled_button(">>", blue(), context.zoom).into());
        },
    }

    buttons.push(button("-", IcedMessage::ZoomOut, blue(), context.zoom).into());
    buttons.push(button("+", IcedMessage::ZoomIn, blue(), context.zoom).into());
    Row::from_vec(buttons).spacing(context.zoom * 8.0).into()
}
