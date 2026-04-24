use super::{
    FeContext,
    Truncation,
    black,
    blue,
    button,
    disabled_button,
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
    ToolCallSuccess,
    Turn,
    TurnId,
    TurnPreview,
    TurnResult,
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
use iced::widget::text_editor::{
    Action as TextEditorAction,
    Content as TextEditorContent,
    Edit as TextEditorEdit,
    TextEditor,
};
use ragit_fs::join3;
use regex::Regex;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

const HELP_MESSAGE: &str = "
This is a neukgu's working directory.

Neukgu reads files, writes files and runs programs inside the directory in order to
accomplish the job you gave to neukgu.

## Interactions

There are 2 ways to interact with neukgu.

1. Pause / Resume neukgu.
2. Interrupt: you can give extra instructions while neukgu is working.

## Context engineering

Below the buttons, you can see a long list of turns. That is the entire trajectory of
neukgu's operations.

You can see a green/red/blue marker on the left of each turn. That has something to do
with context engineering.
";

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
    pub user_response_timeout_counter: Instant,
    pub curr_popup: Option<Popup>,
    pub prev_popup: Option<Popup>,
    pub copy_buffer: Option<String>,
    pub text_editor_content: TextEditorContent,

    // If it's set, it'll display "diff" button in the turn popup.
    pub text_diff: Option<String>,

    // user interaction
    pub is_paused: bool,
    pub pause: Option<bool>,
    pub question_from_user: Option<(u64, String)>,
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

                if let TurnResult::ToolCallSuccess(ToolCallSuccess::Write { diff: Some(diff), .. }) = &turn.turn_result {
                    self.text_diff = Some(diff.to_string());
                }

                else {
                    self.text_diff = None;
                }

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
                let (mut log, mut extension) = load_log(&id, &log_dir)?;
                self.copy_buffer = Some(log.to_string());

                if log.len() > 16384 {
                    log = String::from("The log is too long to display. Copy the log and paste it to your text editor to see the log.");
                    extension = String::from("txt");
                }

                // `LogView::Log` uses a un-editable text-editor to display the content
                // I want to change the content, but I don't know how... this is the best I can do:
                self.text_editor_content.perform(TextEditorAction::SelectAll);
                self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Delete));
                self.text_editor_content.perform(TextEditorAction::Edit(TextEditorEdit::Paste(Arc::new(log.to_string()))));

                self.loaded_log = Some(LogView::Log { log, extension });
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
            Popup::TokenUsage(s) => {
                self.copy_buffer = Some(s.to_string());
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
    ToggleTurnVisibility(TurnId),
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
    TokenUsage(String),
}

#[derive(Clone, Debug)]
pub enum LogView {
    Logs(Vec<String>),
    Log {
        log: String,
        extension: String,
    },
}

pub fn try_boot(no_backend: bool, working_dir: &str, window_size: Size) -> Result<IcedContext, Error> {
    if !no_backend {
        spawn_backend_process(working_dir)?;
    }

    let fe_context = FeContext::load(working_dir)?;
    Ok(IcedContext {
        fe_context: fe_context.clone(),
        window_size,
        turn_view_id: Id::unique(),
        logs_view_id: Id::unique(),
        turn_view_scrolled: AbsoluteOffset { x: 0.0, y: 0.0 },
        hovered_turn: None,
        loaded_turn: None,
        loaded_log: None,
        loaded_image: None,
        user_response_timeout_counter: Instant::now(),
        curr_popup: None,
        prev_popup: None,
        copy_buffer: None,
        text_editor_content: TextEditorContent::new(),
        text_diff: None,
        is_paused: fe_context.is_paused()?,
        pause: None,
        question_from_user: None,
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
                context.question_from_user.take(),
                context.user_response.take(),
            )?;

            if let Some(LogView::Logs(_)) = &context.loaded_log {
                let log_dir = join3(&context.fe_context.working_dir, ".neukgu", "logs")?;
                context.loaded_log = Some(LogView::Logs(load_logs_tail(&log_dir)?));
            }

            if let Some(Popup::TokenUsage(_)) = &context.curr_popup {
                context.curr_popup = Some(Popup::TokenUsage(context.fe_context.get_token_usage().unwrap_or_else(|e| format!("{e:?}"))));
            }

            let llm_request = context.fe_context.get_llm_request()?;

            if let Some((id, _)) = &llm_request {
                if !context.processed_llm_requests.contains(id) {
                    if context.llm_request.is_none() {
                        context.close_popup();
                        context.user_response_timeout_counter = Instant::now();
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
        // "" -> "hidden" -> "pinned"
        IcedMessage::ToggleTurnVisibility(id) => {
            match (context.fe_context.hidden_turns.remove(&id), context.fe_context.pinned_turns.remove(&id)) {
                (true, _) => {
                    context.fe_context.pinned_turns.insert(id);
                },
                (_, true) => {},
                (false, false) => {
                    context.fe_context.hidden_turns.insert(id);
                },
            }

            context.fe_context.interrupt_backend()?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::PauseNeukgu => {
            context.pause = Some(true);
            context.fe_context.interrupt_backend()?;
            return Ok(Task::done(IcedMessage::Tick));
        },
        IcedMessage::ResumeNeukgu => {
            context.pause = Some(false);
        },
        IcedMessage::InterruptNeukgu => {
            context.question_from_user = Some((rand::random::<u64>(), context.text_editor_content.text()));
            context.close_popup();
            context.fe_context.interrupt_backend()?;
            return Ok(Task::done(IcedMessage::Tick));
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

    if context.curr_popup.is_none() && context.llm_request.is_none() {
        turns_scrollable = turns_scrollable.on_scroll(|v| IcedMessage::TurnViewScrolled(v.absolute_offset()));
    }

    let turns_colored = Container::new(turns_scrollable).style(|_| set_bg(black()));
    let full_view = Column::from_vec(vec![
        Container::new(text!("{}", context.fe_context.top_bar())).padding(8).into(),
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
            LogView::Log { extension, .. } => {
                popup(Scrollable::new(TextEditor::new(&context.text_editor_content).highlight(extension, iced::highlighter::Theme::SolarizedDark)).into(), context)
            },
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
                    Some('@') => yellow(),
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

    else if let Some(Popup::TokenUsage(s)) = &context.curr_popup {
        full_view_stacked = Stack::from_vec(vec![
            full_view_stacked,
            popup(Scrollable::new(text!("{s}")).into(), context).into(),
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
    buttons.push(button("Token usage", IcedMessage::OpenPopup { curr: Popup::TokenUsage(context.fe_context.get_token_usage().unwrap_or_else(|e| format!("{e:?}"))), prev: None }, blue()).into());
    buttons.push(button("Help", IcedMessage::OpenPopup { curr: Popup::Help, prev: None }, pink()).into());

    Row::from_vec(buttons).padding(8).spacing(8).into()
}

fn render_turn_preview<'t, 'c, 'm>(index: usize, p: &'t TurnPreview, context: &'c IcedContext) -> Element<'m, IcedMessage> {
    let context_engineering = {
        let color = match context.fe_context.truncation.get(&p.id).unwrap() {
            Truncation::Hidden => red(),
            Truncation::FullRender => green(),
            Truncation::ShortRender => blue(),
        };
        let text = match (context.fe_context.hidden_turns.get(&p.id), context.fe_context.pinned_turns.get(&p.id)) {
            (Some(_), _) => "hidden",
            (_, Some(_)) => "pinned",
            (None, None) => "      ",
        };

        if context.curr_popup.is_none() && context.llm_request.is_none() {
            button(text, IcedMessage::ToggleTurnVisibility(p.id.clone()), color)
        } else {
            disabled_button(text, color)
        }
    };

    let turn_result: Element<IcedMessage> = match p.result {
        TurnResultSummary::ParseError => text!(" (parse-error)").color(red()),
        TurnResultSummary::ToolCallError => text!(" (tool-call-error)").color(yellow()),
        TurnResultSummary::ToolCallSuccess => text!(""),
    }.into();

    let turn_row = Row::from_vec(vec![
        text!("{index:>3}. ").into(),
        text!("[{}]", p.timestamp).into(),
        Column::from_vec(vec![
            Row::from_vec(vec![
                text!("{}", p.preview_title).into(),
                turn_result,
            ]).into(),
            text!("(LLM: {}, TOOL: {})", prettify_time(p.llm_elapsed_ms), prettify_time(p.tool_elapsed_ms)).width(Length::FillPortion(2)).into(),
        ]).width(Length::Fill).into(),
    ]).width(Length::Fill).align_y(Vertical::Center).spacing(4);

    let mut with_color = Container::new(turn_row).padding(8);

    if let Some(id) = &context.hovered_turn && &p.id == id {
        with_color = with_color.style(|_| set_bg(gray(0.45)));
    }

    else {
        with_color = with_color.style(|_| set_bg(gray(0.15)));
    }

    let with_mouse_area: Element<IcedMessage> = if context.curr_popup.is_none() && context.llm_request.is_none() {
        MouseArea::new(with_color)
            .on_enter(IcedMessage::HoverOnTurn(Some(p.id.clone())))
            .on_exit(IcedMessage::HoverOnTurn(None))
            .on_press(IcedMessage::OpenPopup { curr: Popup::Turn((index, p.id.clone())), prev: None })
            .into()
    }

    else {
        with_color.into()
    };

    Row::from_vec(vec![context_engineering.into(), with_mouse_area])
        .width(Length::Fixed(context.window_size.width))
        .align_y(Vertical::Center)
        .spacing(12)
        .into()
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
    let elapsed_secs = Instant::now().duration_since(context.user_response_timeout_counter.clone()).as_secs();

    popup(
        Column::from_vec(vec![
            text!("{}", context.llm_request.as_ref().unwrap().1).into(),
            TextEditor::new(&context.text_editor_content)
                .placeholder("Answer neukgu's question")
                .width(context.window_size.width - 128.0)
                .on_action(|action| IcedMessage::EditText(action))
                .into(),
            Row::from_vec(vec![
                button("Answer", IcedMessage::AnswerLLMRequest, green()).into(),
                button("Dismiss", IcedMessage::DismissLLMRequest, red()).into(),
                text!("{}", context.fe_context.config.user_response_timeout.max(elapsed_secs) - elapsed_secs).into(),
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
