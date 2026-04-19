use super::{
    FeContext,
    Truncation,
    black,
    blue,
    button,
    green,
    pink,
    red,
    set_bg,
    white,
    yellow,
};
use crate::{
    Error,
    ImageId,
    Interrupt,
    LLMToken,
    LogId,
    Turn,
    TurnId,
    TurnPreview,
    TurnResultSummary,
    load_log,
    load_logs_tail,
    prettify_time,
};
use iced::{Background, Color, ContentFit, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, MouseArea, Row, Sensor, Scrollable, Space, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{Handle as ImageHandle, Image, Viewer as ImageViewer};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, scroll_to, snap_to};
use iced::widget::text_editor::{Action as TextEditorAction, Content as TextEditorContent, TextEditor};
use regex::Regex;
use std::sync::LazyLock;

const HELP_MESSAGE: &str = "TODO: Write help message...";

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub fe_context: FeContext,
    pub window_size: Size,
    pub turn_view_id: Id,
    pub logs_view_id: Id,
    pub turn_view_scrolled: AbsoluteOffset,
    pub hovered_turn: Option<TurnId>,
    pub loaded_turn: Option<(usize, Turn)>,
    pub loaded_log: Option<LogView>,
    pub loaded_image: Option<ImageId>,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,

    // If it's set, it'll display "diff" button in the turn popup.
    pub text_diff: Option<String>,

    pub interrupt: Option<Interrupt>,
    pub user_response: Option<(u64, String)>,
    pub interrupt_input_content: TextEditorContent,
}

impl IcedContext {
    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::Turn((index, turn_id)) => {
                let turn = Turn::load(&turn_id)?;
                self.text_diff = self.fe_context.calc_diff(&turn)?;
                self.loaded_turn = Some((index, turn));
            },
            Popup::Interrupt => {
                // There's nothing to load
            },
            Popup::Logs => {
                self.loaded_log = Some(LogView::Logs(load_logs_tail()?));
            },
            Popup::Log(id) => {
                self.loaded_log = Some(LogView::Log(load_log(&id)?));
            },
            Popup::Help => {
                // There's nothing to load
            },
            Popup::Image(id) => {
                self.loaded_image = Some(id);
            },
            Popup::Diff => {
                // It's already loaded in `self.text_diff`
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
pub enum IcedMessage {
    Tick,
    WindowResized(Size),
    TurnViewScrolled(AbsoluteOffset),
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
    EditText(TextEditorAction),
    None,
}

#[derive(Clone, Debug)]
pub enum Popup {
    Turn((usize, TurnId)),
    Interrupt,
    Logs,
    Log(LogId),
    Help,
    Image(ImageId),
    Diff,
}

#[derive(Clone, Debug)]
pub enum LogView {
    Logs(Vec<String>),
    Log(String),
}

pub fn boot() -> IcedContext {
    IcedContext {
        fe_context: FeContext::load().unwrap(),
        window_size: Size::new(0.0, 0.0),
        turn_view_id: Id::unique(),
        logs_view_id: Id::unique(),
        turn_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
        hovered_turn: None,
        loaded_turn: None,
        loaded_log: None,
        loaded_image: None,
        curr_popup: None,
        prev_popup: None,
        text_diff: None,
        interrupt: None,
        user_response: None,
        interrupt_input_content: TextEditorContent::with_text(""),
    }
}

// TODO: too many unwraps here...
pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::Tick => {
            context.fe_context.end_frame(context.interrupt.take(), context.user_response.take()).unwrap();

            if let Some(LogView::Logs(_)) = &context.loaded_log {
                context.loaded_log = Some(LogView::Logs(load_logs_tail().unwrap()));
            }

            context.fe_context.start_frame().unwrap();
        },
        IcedMessage::WindowResized(s) => {
            context.window_size = s;
        },
        IcedMessage::TurnViewScrolled(o) => {
            context.turn_view_scrolled = o;
        },
        IcedMessage::HoverOnTurn(id) => {
            context.hovered_turn = id;
        },
        IcedMessage::OpenPopup { curr, prev } => {
            let mut scrolls: Vec<Task<IcedMessage>> = vec![
                scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled),
            ];

            if let Popup::Logs = &curr {
                scrolls.push(snap_to(context.logs_view_id.clone(), RelativeOffset::END));
            }

            context.open_popup(curr).unwrap();
            context.prev_popup = prev;
            return Task::batch(scrolls);
        },
        IcedMessage::BackPopup => {
            if let Some(prev_popup) = &context.prev_popup {
                let prev_popup = prev_popup.clone();
                context.open_popup(prev_popup).unwrap();
                context.prev_popup = None;
            }
        },
        IcedMessage::CopyPopup => todo!(),
        IcedMessage::ClosePopup => {
            context.close_popup();
            return scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled);
        },
        IcedMessage::PauseNeukgu => {
            context.interrupt = Some(Interrupt::Pause);
        },
        IcedMessage::ResumeNeukgu => {
            context.interrupt = Some(Interrupt::Resume);
        },
        IcedMessage::InterruptNeukgu => {
            context.interrupt = Some(Interrupt::Request { request_id: rand::random::<u64>(), request: context.interrupt_input_content.text() });
            context.close_popup();
            context.interrupt_input_content = TextEditorContent::with_text("");
        },
        IcedMessage::EditText(a) => {
            context.interrupt_input_content.perform(a);
        },
        IcedMessage::None => {},
    }

    Task::none()
}

pub fn view<'a>(context: &'a IcedContext) -> Element<'a, IcedMessage> {
    let mut turns: Vec<Element<IcedMessage>> = context.fe_context.iter_previews().into_iter().enumerate().map(
        |(i, p)| render_turn_preview(i, &p, context)
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

    let mut turns_scrollable = Scrollable::new(turns_stretched).id(context.turn_view_id.clone());

    if context.curr_popup.is_none() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::TurnViewScrolled(v.absolute_offset()));
    }
    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));

    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.fe_context.top_bar().unwrap_or_else(|e| format!("{e:?}")))).padding(8).into(),
        horizontal_bar(context),
        render_buttons(context),
        horizontal_bar(context),
        turns_colored.into(),
    ]);

    let full_view_resizable = Sensor::new(full_view)
        .on_show(|s| IcedMessage::WindowResized(s))
        .on_resize(|s| IcedMessage::WindowResized(s));

    let mut full_view_stacked: Element<IcedMessage> = Container::new(full_view_resizable).into();

    if let Some((index, turn)) = &context.loaded_turn {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_turn(*index, turn, context),
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

    else if let Some(Popup::Diff) = context.curr_popup {
        let diff_view = Column::from_vec(context.text_diff.as_ref().unwrap().lines().map(
            |line| {
                let color = match line.chars().next() {
                    Some('+') => green(),
                    Some('-') => red(),
                    Some('@') => blue(),
                    _ => white(),
                };

                text!("{line}").color(color).into()
            }
        ).collect());
        let diff_view = Scrollable::new(diff_view);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(diff_view.into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Help) = context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(Scrollable::new(text!("{HELP_MESSAGE}")).into(), context).into(),
        ]).into();
    }

    else if let Some(Popup::Interrupt) = context.curr_popup {
        let interrupt_edit = TextEditor::new(&context.interrupt_input_content)
            .placeholder("Say something to neukgu!")
            .on_action(|action| IcedMessage::EditText(action));
        let interrupt_edit = Column::from_vec(vec![
            interrupt_edit.into(),
            button("Send", IcedMessage::InterruptNeukgu, green()).padding(20).into(),
        ]).spacing(20).align_x(Horizontal::Center).width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(interrupt_edit.into(), context).into(),
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'a, 'b>(context: &'a IcedContext) -> Element<'b, IcedMessage> {
    if context.curr_popup.is_some() {
        return Container::new(text!("")).padding(8).into();
    }

    let mut buttons: Vec<Element<IcedMessage>> = if context.fe_context.is_paused() {
        vec![button("Resume", IcedMessage::ResumeNeukgu, blue()).into()]
    } else {
        vec![button("Pause", IcedMessage::PauseNeukgu, blue()).into()]
    };

    buttons.push(button("Interrupt", IcedMessage::OpenPopup { curr: Popup::Interrupt, prev: None }, blue()).into());
    buttons.push(button("See logs", IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }, blue()).into());
    buttons.push(button("Help", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink()).into());

    Row::from_vec(buttons).padding(8).spacing(8).into()
}

fn horizontal_bar<'a, 'b>(context: &'a IcedContext) -> Element<'b, IcedMessage> {
    Container::new(Space::new())
        .style(|_| set_bg(white()))
        .width(Length::Fixed(context.window_size.width))
        .height(Length::Fixed(8.0))
        .into()
}

fn render_turn_preview<'a, 'b, 'c>(index: usize, p: &'a TurnPreview, context: &'b IcedContext) -> Element<'c, IcedMessage> {
    let truncation_color = match context.fe_context.truncation.get(&p.id).unwrap() {
        Truncation::Hidden => red(),
        Truncation::FullRender => green(),
        Truncation::ShortRender => blue(),
    };
    let truncation = Container::new(text!("  ")).style(move |_| set_bg(truncation_color));

    let turn_result: Element<IcedMessage> = match p.result {
        TurnResultSummary::ParseError => text!(" (parse-error)").color(red()),
        TurnResultSummary::ToolCallError => text!(" (tool-call-error)").color(yellow()),
        TurnResultSummary::ToolCallSuccess => text!(""),
    }.into();

    let row = Row::from_vec(vec![
        text!("{index:>3}. ").into(),
        truncation.into(),
        text!("[{}]", p.timestamp).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{}", p.preview_title).into(),
                turn_result,
            ]).into(),
            text!("(LLM: {}, TOOL: {})", prettify_time(p.llm_elapsed_ms), prettify_time(p.tool_elapsed_ms)).width(Length::FillPortion(2)).into(),
        ]).width(Length::Fill).into(),
    ]).width(Length::Fixed(context.window_size.width)).align_y(Vertical::Center).spacing(12);

    let mut with_color = Container::new(row).padding(8);

    if let Some(id) = &context.hovered_turn && &p.id == id {
        with_color = with_color.style(|_| set_bg(Color::from_rgb(0.45, 0.45, 0.45)));
    }

    else {
        with_color = with_color.style(|_| set_bg(Color::from_rgb(0.15, 0.15, 0.15)));
    }

    if context.curr_popup.is_none() {
        MouseArea::new(with_color)
            .on_enter(IcedMessage::HoverOnTurn(Some(p.id.clone())))
            .on_exit(IcedMessage::HoverOnTurn(None))
            .on_press(IcedMessage::OpenPopup { curr: Popup::Turn((index, p.id.clone())), prev: None })
            .into()
    }

    else {
        with_color.into()
    }
}

fn render_turn<'a, 'b, 'c>(index: usize, turn: &'a Turn, context: &'b IcedContext) -> Element<'c, IcedMessage> {
    let mut turn_content = vec![
        text!("# {index}. {}", turn.preview().preview_title).into(),
        text!("<|LLM|>").into(),
        Container::new(
            render_llm_tokens(vec![LLMToken::String(turn.raw_response.to_string())], context)
        ).padding(8).style(|_| set_bg(Color::from_rgb(0.3, 0.3, 0.3))).into(),
        text!("<|result|>").into(),
        Container::new(
            render_llm_tokens(turn.turn_result.to_llm_tokens(&context.fe_context.config), context)
        ).padding(8).style(|_| set_bg(Color::from_rgb(0.3, 0.3, 0.3))).into(),
    ];

    if context.text_diff.is_some() {
        turn_content.push(button(
            "Diff",
            IcedMessage::OpenPopup { curr: Popup::Diff, prev: context.curr_popup.clone() },
            green(),
        ).into());
    }

    let turn_content = Scrollable::new(Column::from_vec(turn_content).padding(8).spacing(8).width(Length::Fill)).width(Length::Fill);
    popup(turn_content.into(), context)
}

pub static LOG_DETAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".*\((\d{7}\-\d{7})\).*").unwrap());

fn render_logs<'a, 'b, 'c>(logs: &'a [String], context: &'b IcedContext) -> Element<'c, IcedMessage> {
    let logs = Scrollable::new(Column::from_vec(
        logs.iter().map(
            |log| {
                if let Some(cap) = LOG_DETAIL_RE.captures(log) {
                    let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                    Row::from_vec(vec![
                        text!("{log}").into(),
                        button("see details", IcedMessage::OpenPopup {
                            curr: Popup::Log(log_id),
                            prev: Some(Popup::Logs),
                        }, green()).into(),
                    ]).align_y(Vertical::Center).spacing(20).into()
                }

                else {
                    text!("{log}").into()
                }
            }
        ).collect()
    ).padding(8).spacing(8).width(Length::Fill)).id(context.logs_view_id.clone()).width(Length::Fill);
    popup(logs.into(), context)
}

fn render_llm_tokens(llm_tokens: Vec<LLMToken>, context: &IcedContext) -> Element<'static, IcedMessage> {
    Column::from_vec(llm_tokens.iter().map(
        |token| match token {
            LLMToken::String(s) => text!("{s}").width(Length::Fill).into(),
            LLMToken::Image(id) => MouseArea::new(
                Image::new(ImageHandle::from_path(id.path().unwrap()))
                    .width(Length::Fixed(300.0))
                    .height(Length::Fixed(300.0))
                    .content_fit(ContentFit::Contain),
            ).on_press(
                IcedMessage::OpenPopup {
                    curr: Popup::Image(*id),
                    prev: context.curr_popup.clone(),
                },
            ).into(),
        }
    ).collect()).width(Length::Fill).into()
}

fn popup<'a, 'b>(element: Element<'a, IcedMessage>, context: &'b IcedContext) -> Element<'a, IcedMessage> {
    let mut buttons: Vec<Element<IcedMessage>> = vec![];

    if context.prev_popup.is_some() {
        buttons.push(button("Back", IcedMessage::BackPopup, blue()).into());
    }

    buttons.push(button("Copy", IcedMessage::CopyPopup, blue()).into());
    buttons.push(button("Close", IcedMessage::ClosePopup, red()).into());

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
