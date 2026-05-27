use super::{black, blue, button, disabled_button, gold, gray, red, white};
use super::slide_rule::{
    self,
    IcedContext as SlideRuleContext,
    IcedMessage as SlideRuleMessage,
};
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
    Status as TextEditorStatus,
    Style as TextEditorStyle,
    TextEditor,
};
use std::sync::Arc;

pub struct IcedContext {
    pub window_size: Size,
    pub alignment: Horizontal,
    pub title: Option<String>,
    pub tab: Tab,
    pub is_expanded: bool,
    pub popup_scroll_id: Id,
    pub text_editor_id: Id,
    pub zoom: f32,

    // context for each tab
    pub text_viewer_context: (String, Option<String>),  // (content, extension)
    pub text_editor_context: String,
    pub image_path: String,
    pub slide_rule_context: SlideRuleContext,

    pub text_editor_content: TextEditorContent,
}

impl IcedContext {
    pub fn new() -> IcedContext {
        IcedContext {
            window_size: Size::new(0.0, 0.0),
            alignment: Horizontal::Right,
            title: None,
            tab: Tab::Hidden,
            is_expanded: true,
            popup_scroll_id: Id::unique(),
            text_editor_id: Id::unique(),
            zoom: 1.0,
            text_viewer_context: (String::new(), None),
            text_editor_context: String::new(),
            image_path: String::new(),
            slide_rule_context: SlideRuleContext::new(),
            text_editor_content: TextEditorContent::new(),
        }
    }

    pub fn set_text_editor_content(&mut self, c: String) {
        self.text_editor_content.perform(TextEditorAction::SelectAll);
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
        self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(c))));
    }

    fn save_context(&mut self) {
        match self.tab {
            Tab::TextView => {
                self.text_viewer_context.0 = self.text_editor_content.text();
            },
            Tab::TextEdit => {
                self.text_editor_context = self.text_editor_content.text();
            },
            Tab::Hidden | Tab::Image | Tab::SlideRule => {},
        }
    }

    pub fn open_content(&mut self, title: Option<String>, content: Content) {
        self.title = title;

        match content {
            Content::Image { path } => {
                self.image_path = path;

                if self.tab != Tab::Image {
                    self.toggle_image();
                }
            },
            Content::Text { content, extension } => {
                self.text_viewer_context = (content.clone(), extension);

                if self.tab != Tab::TextView {
                    self.toggle_text_viewer();
                }

                // We have to call this after `toggle_text_viewer` so that the text_editor_context
                // can be saved safely.
                self.set_text_editor_content(content);
            },
        }
    }

    pub fn toggle_text_viewer(&mut self) {
        self.save_context();

        if self.tab == Tab::TextView {
            self.tab = Tab::Hidden;
        } else {
            self.tab = Tab::TextView;
            self.set_text_editor_content(self.text_viewer_context.0.to_string());
        }
    }

    pub fn toggle_text_editor(&mut self) {
        self.save_context();

        if self.tab == Tab::TextEdit {
            self.tab = Tab::Hidden;
        } else {
            self.tab = Tab::TextEdit;
            self.set_text_editor_content(self.text_editor_context.to_string());
        }
    }

    pub fn toggle_image(&mut self) {
        self.save_context();

        if self.tab == Tab::Image {
            self.tab = Tab::Hidden;
        } else {
            self.tab = Tab::Image;
        }
    }

    pub fn toggle_slide_rule(&mut self) {
        self.save_context();

        if self.tab == Tab::SlideRule {
            self.tab = Tab::Hidden;
        } else {
            self.tab = Tab::SlideRule;
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },
    WindowResized(Size),
    ToggleExpand,
    SetAlignment(Horizontal),
    UpdateSlideRule(SlideRuleMessage),
    EditText(TextEditorAction),
    ZoomIn,
    ZoomOut,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Hidden,
    TextView,
    TextEdit,
    Image,
    SlideRule,
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
        IcedMessage::UpdateSlideRule(m) => {
            return slide_rule::update(&mut context.slide_rule_context, m).map(IcedMessage::UpdateSlideRule);
        },
        IcedMessage::EditText(a) => {
            context.text_editor_content.perform(a);
        },
        IcedMessage::ZoomIn => {
            context.zoom = context.zoom.min(2.4) + 0.1;
        },
        IcedMessage::ZoomOut => {
            context.zoom = context.zoom.max(0.2) - 0.1;
        },
        IcedMessage::Close => {
            context.tab = Tab::Hidden;
        },
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
    let mut content: Element<IcedMessage> = match context.tab {
        Tab::Hidden => unreachable!(),
        Tab::TextView => TextEditor::new(&context.text_editor_content)
            .id(context.text_editor_id.clone())
            .width(context.window_size.width)
            .size(context.zoom * 14.0)
            .highlight(
                &if let Some(extension) = &context.text_viewer_context.1 { extension.to_string() } else { String::from("txt") },
                iced::highlighter::Theme::InspiredGitHub,
            )
            .style(|_, _| TextEditorStyle {
                background: Background::Color(gray(0.85)),
                border: Border::default(),
                placeholder: gray(0.15),
                value: black(),
                selection: gray(0.4),
            })
            .into(),
        Tab::TextEdit => TextEditor::new(&context.text_editor_content)
            .id(context.text_editor_id.clone())
            .width(context.window_size.width)
            .min_height(context.zoom * 200.0)
            .size(context.zoom * 14.0)
            .on_action(IcedMessage::EditText)
            .style(|_, status| {
                let border = match status {
                    TextEditorStatus::Hovered => Border {
                        color: black(),
                        width: 2.0,
                        ..Border::default()
                    },
                    TextEditorStatus::Focused { .. } => Border {
                        color: gold(),
                        width: 2.0,
                        ..Border::default()
                    },
                    _ => Border::default(),
                };

                TextEditorStyle {
                    background: Background::Color(gray(0.85)),
                    border,
                    placeholder: gray(0.15),
                    value: black(),
                    selection: gray(0.4),
                }
            })
            .into(),
        Tab::Image => ImageViewer::new(ImageHandle::from_path(&context.image_path)).into(),
        Tab::SlideRule => slide_rule::view(&context.slide_rule_context, context.window_size, context.zoom).map(IcedMessage::UpdateSlideRule),
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
