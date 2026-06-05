use chrono::Local;
use crate::{
    ApiLog,
    Config,
    Context,
    Error,
    LLMToken,
    LogEntry,
    RawResponse,
    Request,
    Thinking,
    ToolCallSuccess,
    ToolKind,
    TurnKind,
    TurnResult,
    prompt,
    request,
};
use ragit_fs::{
    WriteMode,
    basename,
    join3,
    join4,
    read_dir,
    remove_file,
    write_bytes,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Instant;

// How the interruption mechanism works.
//
// 1. There are files in `.neukgu/interruptions/`.
//    - Each file's file name is the timestamp of the file's creation time.
// 2. If the backend sees an interruption file in the directory and the
//    interruption is less than 5 seconds old, the backend immediately
//    halts the current turn.
// 3. Whenever the backend checks the interruption directory, it empties
//    the directory.

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum InterruptKind {
    Question,
    Instruction,
}

impl InterruptKind {
    pub fn another(&self) -> Self {
        match self {
            InterruptKind::Question => InterruptKind::Instruction,
            InterruptKind::Instruction => InterruptKind::Question,
        }
    }
}

impl From<InterruptKind> for TurnKind {
    fn from(k: InterruptKind) -> TurnKind {
        match k {
            InterruptKind::Question => TurnKind::UserQuestion,
            InterruptKind::Instruction => TurnKind::UserInstruction,
        }
    }
}

pub fn check_interruption(working_dir: &str) -> Result<bool, Error> {
    let interruption_dir = join3(working_dir, ".neukgu", "interruptions")?;
    let now = Local::now().timestamp_millis();
    let mut files_to_remove = vec![];
    let mut interruption = false;

    for file in read_dir(&interruption_dir, false)? {
        let basename = basename(&file)?;
        files_to_remove.push(file);

        if let Ok(n) = basename.parse::<i64>() {
            if n + 5000 > now {
                interruption = true;
            }
        }
    }

    for file in files_to_remove.iter() {
        remove_file(file)?;
    }

    Ok(interruption)
}

pub fn interrupt_be(working_dir: &str) -> Result<(), Error> {
    let now = Local::now().timestamp_millis().to_string();
    write_bytes(
        &join4(working_dir, ".neukgu", "interruptions", &now)?,
        b"",
        WriteMode::Atomic,
    )?;
    Ok(())
}

impl Context {
    pub async fn process_interrupt_from_user(
        &mut self,
        id: u64,
        interrupt_kind: InterruptKind,
        interrupt: String,
        config: &Config,
    ) -> Result<(), Error> {
        match interrupt_kind {
            InterruptKind::Question => {
                let started_at = Instant::now();
                let answer = self.answer_user_question(&interrupt, config).await?;
                self.curr_raw_response = Some(RawResponse {
                    thinking: None,
                    response: String::new(),
                    elapsed_ms: Instant::now().duration_since(started_at).as_millis() as u64,
                    logs: ApiLog::new(),
                });
                let turn_result = TurnResult::ToolCallSuccess(ToolCallSuccess::QuestionFromUser {
                    q: interrupt,
                    a: answer,
                });
                self.finish_turn(
                    None,
                    turn_result,
                    0,
                    config,
                    interrupt_kind.into(),
                )?;
            },
            InterruptKind::Instruction => {
                let q = "
<ask>
<to>user</to>
<question>Do you have any feedbacks?</question>
</ask>
";
                // Let's make sure that the schema is correct.
                self.curr_raw_response = Some(RawResponse {
                    thinking: None,
                    response: q.to_string(),
                    elapsed_ms: 0,
                    logs: ApiLog::new(),
                });
                let parse_result = crate::parse::parse(q.as_bytes(), &[ToolKind::Ask]).unwrap();

                let turn_result = TurnResult::ToolCallSuccess(ToolCallSuccess::InstructionFromUser(interrupt));
                self.finish_turn(
                    Some(parse_result),
                    turn_result,
                    0,
                    config,
                    interrupt_kind.into(),
                )?;
            },
        }

        self.completed_interrupts_from_user.insert(id);
        Ok(())
    }

    pub async fn answer_user_question(&mut self, question: &str, config: &Config) -> Result<String, Error> {
        self.logger.log(LogEntry::QuestionFromUserStart(question.to_string()))?;
        let (mut history, mut last_turn) = self.fit_history_to_llm_context(config)?;
        last_turn.push(LLMToken::String(String::from("\n\nThe user has a question.")));
        history.push(request::Turn {
            query: last_turn,
            response: String::from("Okay, what is it?"),
        });

        let request = Request {
            model: config.agents.small,
            system_prompt: prompt::user_question_system_prompt(),
            history,
            query: vec![LLMToken::String(prompt::user_question_prompt(question))],
            enable_web_search: false,
            thinking: Thinking::Disabled,
        };
        let response = request.request(&config.request_config(), &self.working_dir, true, &self.logger).await?;
        self.logger.log(LogEntry::QuestionFromUserEnd)?;

        match Regex::new(r"(?s)<answer>(.+)</answer>").unwrap().captures(&response.response) {
            Some(cap) => Ok(cap.get(1).unwrap().as_str().trim().to_string()),
            None => Ok(response.response),
        }
    }
}
