use super::{FeContext, Truncation, spawn_backend_process};
use crate::{
    Error,
    ImageId,
    Interrupt,
    StringOrImage,
    Turn,
    TurnId,
    TurnPreview,
    TurnResultSummary,
    load_log_tail,
    prettify_time,
};
use iced::{Background, Color, ContentFit, Element, Font, Length, Size, Subscription, Theme};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::keyboard::{self, Event as KeyboardEvent, Key, key::Named as NamedKey};
use iced::time::{self, Duration};
use iced::widget::{Column, MouseArea, Row, Sensor, Scrollable, Space, Stack, text};
use iced::widget::button::{Button, Status as ButtonStatus, Style as ButtonStyle};
use iced::widget::container::{Container, Style};
use iced::widget::image::{Handle as ImageHandle, Image, Viewer as ImageViewer};
use ragit_fs::{exists, join3, read_string};
use regex::Regex;
use std::sync::LazyLock;

pub fn run(no_backend: bool) -> Result<(), Error> {
    if !no_backend {
        spawn_backend_process()?;
    }

    iced::application(boot, update, view)
        .theme(Theme::Dark)
        .default_font(Font::MONOSPACE)
        .subscription(|_| Subscription::batch([
            time::every(Duration::from_millis(1_000)).map(|_| Message::Tick),
            keyboard::listen().map(|key| match key {
                KeyboardEvent::KeyPressed { key: Key::Named(NamedKey::Escape), .. } => Message::ClosePopup,
                _ => Message::None,
            }),
        ]))
        .run()
        .unwrap();
    Ok(())
}

#[derive(Clone, Debug)]
struct GuiContext {
    pub fe_context: FeContext,
    pub window_size: Size,
    pub hovered_turn: Option<TurnId>,
    pub loaded_turn: Option<Turn>,
    pub loaded_log: Option<LogView>,
    pub loaded_image: Option<ImageId>,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub interrupt: Option<Interrupt>,
    pub user_response: Option<(u64, String)>,
}

impl GuiContext {
    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::Turn(id) => {
                let turn = Turn::load(&id)?;
                self.loaded_turn = Some(turn);
            },
            Popup::Logs => {
                self.loaded_log = Some(LogView::Logs(load_log_tail()?));
            },
            Popup::Log(log_path) => {
                self.loaded_log = Some(LogView::Log(read_string(&log_path)?));
            },
            Popup::Image(id) => {
                self.loaded_image = Some(id);
            },
        }

        Ok(())
    }

    pub fn close_popup(&mut self) {
        self.hovered_turn = None;
        self.loaded_turn = None;
        self.loaded_log = None;
        self.loaded_image = None;
        self.curr_popup = None;
    }
}

#[derive(Clone, Debug)]
enum Message {
    Tick,
    WindowResized(Size),
    HoverOnTurn(Option<TurnId>),
    OpenPopup {
        curr: Popup,
        prev: Option<Popup>,
    },
    BackPopup,
    CopyPopup,
    ClosePopup,
    PauseNeukgu,
    ResumeNeukgu,
    InterruptNeukgu,
    None,
}

#[derive(Clone, Debug)]
enum Popup {
    Turn(TurnId),
    Logs,
    Log(String),
    Image(ImageId),
}

#[derive(Clone, Debug)]
enum LogView {
    Logs(Vec<String>),
    Log(String),
}

fn boot() -> GuiContext {
    GuiContext {
        fe_context: FeContext::load().unwrap(),
        window_size: Size::new(0.0, 0.0),
        hovered_turn: None,
        loaded_turn: None,
        loaded_log: None,
        loaded_image: None,
        curr_popup: None,
        prev_popup: None,
        interrupt: None,
        user_response: None,
    }
}

// TODO: too many unwraps here...
fn update(context: &mut GuiContext, message: Message) {
    match message {
        Message::Tick => {
            context.fe_context.end_frame(context.interrupt.take(), context.user_response.take()).unwrap();

            if let Some(LogView::Logs(_)) = &context.loaded_log {
                context.loaded_log = Some(LogView::Logs(load_log_tail().unwrap()));
            }

            context.fe_context.start_frame().unwrap();
        },
        Message::WindowResized(s) => {
            context.window_size = s;
        },
        Message::HoverOnTurn(id) => {
            context.hovered_turn = id;
        },
        Message::OpenPopup { curr, prev } => {
            context.open_popup(curr).unwrap();
            context.prev_popup = prev;
        },
        Message::BackPopup => {
            if let Some(prev_popup) = &context.prev_popup {
                let prev_popup = prev_popup.clone();
                context.open_popup(prev_popup).unwrap();
                context.prev_popup = None;
            }
        },
        Message::CopyPopup => todo!(),
        Message::ClosePopup => {
            context.close_popup();
        },
        Message::PauseNeukgu => {
            context.interrupt = Some(Interrupt::Pause);
        },
        Message::ResumeNeukgu => {
            context.interrupt = Some(Interrupt::Resume);
        },
        Message::InterruptNeukgu => {},
        Message::None => {},
    }
}

fn view(context: &GuiContext) -> Element<'_, Message> {
    let mut turns: Vec<Element<Message>> = context.fe_context.iter_previews().into_iter().map(
        |p| render_turn_preview(&p, context)
    ).collect();

    turns.push(text!("{}", context.fe_context.curr_status()).into());

    if let Some(error) = context.fe_context.curr_error() {
        turns.push(text!("{error}").color(red()).into());
    }

    // It makes rooms for popups when there're not enough turns.
    turns.push(text!("").width(Length::Fixed(800.0)).height(Length::Fixed(800.0)).into());

    let turns_stretched = Column::from_vec(turns)
        .padding(12)
        .spacing(12);

    let turns_scrollable = Scrollable::new(turns_stretched);
    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));

    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.fe_context.top_bar())).padding(8).into(),
        horizontal_bar(context),
        render_buttons(context),
        horizontal_bar(context),
        turns_colored.into(),
    ]);

    let full_view_resizable = Sensor::new(full_view)
        .on_show(|s| Message::WindowResized(s))
        .on_resize(|s| Message::WindowResized(s));

    let mut full_view_stacked: Element<Message> = Container::new(full_view_resizable).into();

    if let Some(turn) = &context.loaded_turn {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_turn(turn, context),
        ]).into()
    }

    else if let Some(loaded_log) = &context.loaded_log {
        let view = match loaded_log {
            LogView::Logs(logs) => render_logs(logs, context),
            LogView::Log(log) => popup(Scrollable::new(text!("{log}")).into(), context),
        };

        full_view_stacked = Stack::from_vec(vec![full_view_stacked, view]).into();
    }

    else if let Some(loaded_image) = context.loaded_image {
        let image_view: Element<_> = popup(
            ImageViewer::new(ImageHandle::from_path(loaded_image.path().unwrap())).content_fit(ContentFit::Contain).into(),
            context,
        ).into();

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            image_view,
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'a, 'b>(context: &'a GuiContext) -> Element<'b, Message> {
    if context.curr_popup.is_some() {
        return Container::new(text!("")).padding(8).into();
    }

    let mut buttons: Vec<Element<Message>> = if context.fe_context.is_paused() {
        vec![button("Resume", Message::ResumeNeukgu, blue()).into()]
    } else {
        vec![button("Pause", Message::PauseNeukgu, blue()).into()]
    };

    buttons.push(button("Interrupt", Message::InterruptNeukgu, blue()).into());
    buttons.push(button("See logs", Message::OpenPopup { curr: Popup::Logs, prev: None }, blue()).into());

    Row::from_vec(buttons).padding(8).spacing(8).into()
}

fn horizontal_bar<'a, 'b>(context: &'a GuiContext) -> Element<'b, Message> {
    Container::new(Space::new())
        .style(|_| set_bg(white()))
        .width(Length::Fixed(context.window_size.width))
        .height(Length::Fixed(8.0))
        .into()
}

fn render_turn_preview<'a, 'b, 'c>(p: &'a TurnPreview, context: &'b GuiContext) -> Element<'c, Message> {
    let truncation_color = match context.fe_context.truncation.get(&p.id).unwrap() {
        Truncation::Hidden => red(),
        Truncation::FullRender => green(),
        Truncation::ShortRender => blue(),
    };
    let truncation = Container::new(text!("  ")).style(move |_| set_bg(truncation_color));

    let turn_result: Element<Message> = match p.result {
        TurnResultSummary::ParseError => text!(" (parse-error)").color(red()),
        TurnResultSummary::ToolCallError => text!(" (tool-call-error)").color(yellow()),
        TurnResultSummary::ToolCallSuccess => text!(""),
    }.into();

    let row = Row::from_vec(vec![
        truncation.into(),
        text!("[{}]", p.timestamp).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{}", p.preview_title).into(),
                turn_result,
            ]).into(),
            text!("(LLM: {}, TOOL: {})", prettify_time(p.llm_elapsed_ms), prettify_time(p.tool_elapsed_ms)).width(Length::FillPortion(2)).into(),
        ]).width(Length::Fill).into(),
    ]).width(Length::Fixed(context.window_size.width)).spacing(12);

    let mut with_color = Container::new(row).padding(8);

    if let Some(id) = &context.hovered_turn && &p.id == id {
        with_color = with_color.style(|_| set_bg(Color::from_rgb(0.45, 0.45, 0.45)));
    }

    else {
        with_color = with_color.style(|_| set_bg(Color::from_rgb(0.15, 0.15, 0.15)));
    }

    if context.curr_popup.is_none() {
        MouseArea::new(with_color)
            .on_enter(Message::HoverOnTurn(Some(p.id.clone())))
            .on_exit(Message::HoverOnTurn(None))
            .on_press(Message::OpenPopup { curr: Popup::Turn(p.id.clone()), prev: None })
            .into()
    }

    else {
        with_color.into()
    }
}

fn render_turn<'a, 'b, 'c>(turn: &'a Turn, context: &'b GuiContext) -> Element<'c, Message> {
    let turn_content = vec![
        text!("<|LLM|>").into(),
        Container::new(
            render_llm_tokens(vec![StringOrImage::String(turn.raw_response.to_string())], context)
        ).padding(8).style(|_| set_bg(Color::from_rgb(0.3, 0.3, 0.3))).into(),
        text!("<|result|>").into(),
        Container::new(
            render_llm_tokens(turn.turn_result.to_llm_tokens(&context.fe_context.config), context)
        ).padding(8).style(|_| set_bg(Color::from_rgb(0.3, 0.3, 0.3))).into(),
    ];
    let turn_content = Scrollable::new(Column::from_vec(turn_content).padding(8).spacing(8).width(Length::Fill)).width(Length::Fill);
    popup(turn_content.into(), context)
}

pub static LOG_DETAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*\((\d{7}\-\d{7})\).*").unwrap());

fn render_logs<'a, 'b, 'c>(logs: &'a [String], context: &'b GuiContext) -> Element<'c, Message> {
    let logs = Scrollable::new(Column::from_vec(
        logs.iter().map(
            |log| {
                if let Some(cap) = LOG_DETAIL_RE.captures(log) {
                    let log_id = cap.get(1).unwrap().as_str().to_string();
                    let mut log_path = None;

                    for ext in ["rs", "json", "txt"] {
                        let candidate = join3(".neukgu", "logs", &format!("{log_id}.{ext}")).unwrap();

                        if exists(&candidate) {
                            log_path = Some(candidate);
                            break;
                        }
                    }

                    Row::from_vec(vec![
                        text!("{log}").into(),
                        button("see details", Message::OpenPopup {
                            curr: Popup::Log(log_path.unwrap()),
                            prev: Some(Popup::Logs),
                        }, green()).into(),
                    ]).align_y(Vertical::Center).spacing(20).into()
                }

                else {
                    text!("{log}").into()
                }
            }
        ).collect()
    ).padding(8).spacing(8).width(Length::Fill)).width(Length::Fill);
    popup(logs.into(), context)
}

fn render_llm_tokens(llm_tokens: Vec<StringOrImage>, context: &GuiContext) -> Element<'static, Message> {
    Column::from_vec(llm_tokens.iter().map(
        |token| match token {
            StringOrImage::String(s) => text!("{s}").width(Length::Fill).into(),
            StringOrImage::Image(id) => MouseArea::new(
                Image::new(ImageHandle::from_path(id.path().unwrap()))
                    .width(Length::Fixed(300.0))
                    .height(Length::Fixed(300.0))
                    .content_fit(ContentFit::Contain),
            ).on_press(
                Message::OpenPopup {
                    curr: Popup::Image(*id),
                    prev: context.curr_popup.clone(),
                },
            ).into(),
        }
    ).collect()).width(Length::Fill).into()
}

fn button<'s>(name: &'s str, message: Message, solid_color: Color) -> Button<'s, Message> {
    Button::new(name)
        .style(move |_, status| {
            let bg_color = match status {
                ButtonStatus::Hovered => Color::from_rgba(solid_color.r, solid_color.g, solid_color.b, 0.5),
                _ => solid_color,
            };

            ButtonStyle {
                background: Some(Background::Color(bg_color)),
                text_color: white(),
                border: Border {
                    color: black(),
                    width: 0.0,
                    radius: Radius::new(4.0),
                },
                ..ButtonStyle::default()
            }
        })
        .padding(8)
        .on_press(message)
}

fn popup<'a, 'b>(element: Element<'a, Message>, context: &'b GuiContext) -> Element<'a, Message> {
    let mut buttons: Vec<Element<Message>> = vec![];

    if context.prev_popup.is_some() {
        buttons.push(button("Back", Message::BackPopup, blue()).into());
    }

    buttons.push(button("Copy", Message::CopyPopup, blue()).into());
    buttons.push(button("Close", Message::ClosePopup, red()).into());

    Container::new(
        Container::new(Column::from_vec(vec![
            Row::from_vec(buttons).padding(8).spacing(8).into(),
            element,
        ]).width(Length::Fill)).style(
            |_| Style {
                background: Some(Background::Color(black())),
                border: Border {
                    color: white(),
                    width: 4.0,
                    radius: Radius::new(8.0),
                },
                ..Style::default()
            }
        )
        .padding(8.0)
        .width(Length::Fill)
    )
    .style(|_| set_bg(Color::from_rgba(0.0, 0.0, 0.0, 0.5)))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(32.0)
    .into()
}

fn set_bg(color: Color) -> Style {
    Style {
        background: Some(Background::Color(color)),
        ..Style::default()
    }
}

fn white() -> Color {
    Color::from_rgb(1.0, 1.0, 1.0)
}

fn black() -> Color {
    Color::from_rgb(0.0, 0.0, 0.0)
}

fn red() -> Color {
    Color::from_rgb(0.8, 0.2, 0.2)
}

fn green() -> Color {
    Color::from_rgb(0.2, 0.8, 0.2)
}

fn blue() -> Color {
    Color::from_rgb(0.2, 0.2, 0.8)
}

fn yellow() -> Color {
    Color::from_rgb(0.8, 0.8, 0.2)
}
