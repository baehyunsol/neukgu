use super::{
    FeContext,
    Truncation,
    black,
    blue,
    button,
    gray,
    green,
    horizontal_bar,
    pink,
    red,
    set_bg,
    spawn_backend_process,
    white,
    yellow,
};
use crate::{
    Error,
    ImageId,
    LLMToken,
    LogId,
    Turn,
    TurnId,
    TurnPreview,
    TurnResultSummary,
    UserResponse,
    load_log,
    load_logs_tail,
    prettify_time,
    stringify_llm_tokens,
};
use iced::{Background, Color, ContentFit, Element, Length, Size, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, MouseArea, Row, Scrollable, Sensor, Stack, text};
use iced::widget::container::{Container, Style};
use iced::widget::image::{Handle as ImageHandle, Image, Viewer as ImageViewer};
use iced::widget::operation::{AbsoluteOffset, RelativeOffset, scroll_to, snap_to};
use iced::widget::text_editor::{Action as TextEditorAction, Content as TextEditorContent, TextEditor};
use ragit_fs::join3;
use regex::Regex;
use std::collections::HashSet;
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
    pub copy_buffer: Option<String>,
    pub text_editor_content: TextEditorContent,

    // If it's set, it'll display "diff" button in the turn popup.
    pub text_diff: Option<String>,

    // user interaction
    pub is_paused: bool,
    pub pause: Option<bool>,
    pub user_interrupt: Option<(u64, String)>,
    pub llm_request: Option<(u64, String)>,
    pub processed_llm_requests: HashSet<u64>,
    pub user_response: Option<(u64, UserResponse)>,
}

impl IcedContext {
    pub fn open_popup(&mut self, popup: Popup) -> Result<(), Error> {
        self.close_popup();
        self.curr_popup = Some(popup.clone());

        match popup {
            Popup::Turn((index, turn_id)) => {
                let turn = Turn::load(&turn_id, &self.fe_context.working_dir)?;
                self.text_diff = self.fe_context.calc_diff(&turn)?;
                self.copy_buffer = Some(format!(
"# {index}. {}

<|LLM|>

{}

<|result|>

{}",
                    turn.preview().preview_title,
                    turn.raw_response,
                    stringify_llm_tokens(&turn.turn_result.to_llm_tokens(&self.fe_context.config)),
                ));
                self.loaded_turn = Some((index, turn));
            },
            // There's nothing to load
            Popup::Interrupt => {},
            Popup::Logs => {
                let log_dir = join3(&self.fe_context.working_dir, ".neukgu", "logs")?;
                let logs = load_logs_tail(&log_dir)?;
                self.copy_buffer = Some(logs.join("\n"));
                self.loaded_log = Some(LogView::Logs(logs));
            },
            Popup::Log(id) => {
                let log_dir = join3(&self.fe_context.working_dir, ".neukgu", "logs")?;
                let log = load_log(&id, &log_dir)?;
                self.copy_buffer = Some(log.to_string());
                self.loaded_log = Some(LogView::Log(log));
            },
            Popup::Help => {
                // There's nothing to load
                self.copy_buffer = Some(HELP_MESSAGE.to_string());
            },
            Popup::Image(id) => {
                self.loaded_image = Some(id);
            },
            // It's already loaded in `self.text_diff`
            Popup::Diff => {
                self.copy_buffer = self.text_diff.clone();
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
        self.copy_buffer = None;
        self.text_editor_content = TextEditorContent::with_text("");
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
    ClosePopup,
    CopyToClipboard,
    PauseNeukgu,
    ResumeNeukgu,
    InterruptNeukgu,
    AnswerLLMRequest,
    DismissLLMRequest,
    EditText(TextEditorAction),
    Error(String),
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

pub fn try_boot(no_backend: bool, working_dir: &str) -> Result<IcedContext, Error> {
    if !no_backend {
        spawn_backend_process(working_dir)?;
    }

    let fe_context = FeContext::load(working_dir)?;
    Ok(IcedContext {
        fe_context: fe_context.clone(),
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
        copy_buffer: None,
        text_editor_content: TextEditorContent::with_text(""),
        text_diff: None,
        is_paused: fe_context.is_paused()?,
        pause: None,
        user_interrupt: None,
        llm_request: None,
        processed_llm_requests: HashSet::new(),
        user_response: None,
    })
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match try_update(context, message) {
        Ok(t) => t,
        Err(e) => Task::done(IcedMessage::Error(format!("{e:?}"))),
    }
}

fn try_update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick => {
            context.fe_context.end_frame(
                context.pause.take(),
                context.user_interrupt.take(),
                context.user_response.take(),
            )?;

            if let Some(LogView::Logs(_)) = &context.loaded_log {
                let log_dir = join3(&context.fe_context.working_dir, ".neukgu", "logs")?;
                context.loaded_log = Some(LogView::Logs(load_logs_tail(&log_dir)?));
            }

            let llm_request = context.fe_context.get_llm_request()?;

            if let Some((id, _)) = &llm_request {
                if !context.processed_llm_requests.contains(id) {
                    if context.llm_request.is_none() {
                        context.close_popup();
                    }

                    context.llm_request = llm_request;
                }
            }

            else if context.llm_request.is_some() {
                context.llm_request = None;
                context.close_popup();
            }

            context.is_paused = context.fe_context.is_paused()?;
            context.fe_context.start_frame()?;
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

            context.open_popup(curr)?;
            context.prev_popup = prev;
            return Ok(Task::batch(scrolls));
        },
        IcedMessage::BackPopup => {
            if let Some(prev_popup) = &context.prev_popup {
                let prev_popup = prev_popup.clone();
                context.open_popup(prev_popup)?;
                context.prev_popup = None;
            }
        },
        IcedMessage::ClosePopup => {
            context.close_popup();
            return Ok(scroll_to(context.turn_view_id.clone(), context.turn_view_scrolled));
        },
        IcedMessage::CopyToClipboard => {
            return Ok(iced::clipboard::write(context.copy_buffer.clone().unwrap()));
        },
        IcedMessage::PauseNeukgu => {
            context.pause = Some(true);
        },
        IcedMessage::ResumeNeukgu => {
            context.pause = Some(false);
        },
        IcedMessage::InterruptNeukgu => {
            context.user_interrupt = Some((rand::random::<u64>(), context.text_editor_content.text()));
            context.close_popup();
        },
        IcedMessage::AnswerLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Answer(context.text_editor_content.text())));
            context.text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::DismissLLMRequest => {
            let Some((id, _)) = context.llm_request.take() else { unreachable!() };
            context.processed_llm_requests.insert(id);
            context.user_response = Some((id, UserResponse::Reject));
            context.text_editor_content = TextEditorContent::with_text("");
        },
        IcedMessage::EditText(a) => {
            context.text_editor_content.perform(a);
        },
        IcedMessage::Error(_) => unreachable!(),
    }

    Ok(Task::none())
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
        .padding(8)
        .spacing(8);

    let mut turns_scrollable = Scrollable::new(turns_stretched).id(context.turn_view_id.clone());

    if context.curr_popup.is_none() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::TurnViewScrolled(v.absolute_offset()));
    }

    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));
    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.fe_context.top_bar().unwrap_or_else(|e| format!("{e:?}")))).padding(8).into(),
        horizontal_bar(context.window_size.width),
        render_buttons(context),
        horizontal_bar(context.window_size.width),
        turns_colored.into(),
    ]);

    let full_view_resizable = Sensor::new(full_view)
        .on_show(|s| IcedMessage::WindowResized(s))
        .on_resize(|s| IcedMessage::WindowResized(s));

    let mut full_view_stacked: Element<IcedMessage> = Container::new(full_view_resizable).into();

    if context.llm_request.is_some() {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_ask_to_user_popup(context),
        ]).into();
    }

    else if let Some((index, turn)) = &context.loaded_turn {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            render_turn(*index, turn, context),
        ]).into();
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
            ImageViewer::new(ImageHandle::from_path(loaded_image.path(&context.fe_context.working_dir).unwrap())).content_fit(ContentFit::Contain).into(),
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
        let text_editor = TextEditor::new(&context.text_editor_content)
            .placeholder("Say something to neukgu!")
            .on_action(|action| IcedMessage::EditText(action));
        let interrupt_edit = Column::from_vec(vec![
            text_editor.into(),
            button("Send", IcedMessage::InterruptNeukgu, green()).padding(20).into(),
        ]).spacing(20).align_x(Horizontal::Center).width(Length::Fill);

        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(interrupt_edit.into(), context).into(),
        ]).into();
    }

    full_view_stacked
}

fn render_buttons<'c, 'm>(context: &'c IcedContext) -> Element<'m, IcedMessage> {
    if context.curr_popup.is_some() || context.llm_request.is_some() {
        return Container::new(text!("")).padding(8).into();
    }

    let mut buttons: Vec<Element<IcedMessage>> = if context.is_paused {
        vec![button("Resume", IcedMessage::ResumeNeukgu, blue()).into()]
    } else {
        vec![button("Pause", IcedMessage::PauseNeukgu, blue()).into()]
    };

    buttons.push(button("Interrupt", IcedMessage::OpenPopup { curr: Popup::Interrupt, prev: None }, blue()).into());
    buttons.push(button("See logs", IcedMessage::OpenPopup { curr: Popup::Logs, prev: None }, blue()).into());
    buttons.push(button("Help", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink()).into());

    Row::from_vec(buttons).padding(8).spacing(8).into()
}

fn render_turn_preview<'t, 'c, 'm>(index: usize, p: &'t TurnPreview, context: &'c IcedContext) -> Element<'m, IcedMessage> {
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
        with_color = with_color.style(|_| set_bg(gray(0.45)));
    }

    else {
        with_color = with_color.style(|_| set_bg(gray(0.15)));
    }

    if context.curr_popup.is_none() && context.llm_request.is_none() {
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
        ).padding(8).style(|_| set_bg(gray(0.3))).into(),
        text!("<|result|>").into(),
        Container::new(
            render_llm_tokens(turn.turn_result.to_llm_tokens(&context.fe_context.config), context)
        ).padding(8).style(|_| set_bg(gray(0.3))).into(),
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
                Image::new(ImageHandle::from_path(id.path(&context.fe_context.working_dir).unwrap()))
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

fn render_ask_to_user_popup<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    popup(
        Column::from_vec(vec![
            text!("{}", context.llm_request.as_ref().unwrap().1).into(),
            TextEditor::new(&context.text_editor_content)
                .placeholder("Answer neukgu's question")
                .on_action(|action| IcedMessage::EditText(action))
                .into(),
            Row::from_vec(vec![
                button("Answer", IcedMessage::AnswerLLMRequest, green()).into(),
                button("Dismiss", IcedMessage::DismissLLMRequest, red()).into(),
            ]).spacing(20).into(),
        ]).padding(20).spacing(20).into(),
        context,
    )
}

fn popup<'a, 'b>(element: Element<'a, IcedMessage>, context: &'b IcedContext) -> Element<'a, IcedMessage> {
    let mut buttons: Vec<Element<IcedMessage>> = vec![];

    if context.curr_popup.is_some() {
        buttons.push(button("Close", IcedMessage::ClosePopup, red()).into());
    }

    if context.prev_popup.is_some() {
        buttons.push(button("Back", IcedMessage::BackPopup, blue()).into());
    }

    if context.copy_buffer.is_some() {
        buttons.push(button("Copy", IcedMessage::CopyToClipboard, blue()).into());
    }

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
