use async_std::task::sleep;
use crate::{
    ApiProvider,
    Error,
    ImageId,
    Logger,
    LogEntry,
    LogId,
    Model,
    Response,
    check_interruption,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

mod anthropic;
mod mock;
mod openai;

pub use mock::{MockState, reset_mock_state, revert_mock_state};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpRequest {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Request {
    pub model: Model,
    pub system_prompt: String,
    pub history: Vec<Turn>,
    pub query: Vec<LLMToken>,
    pub enable_web_search: bool,
    pub thinking: Thinking,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Thinking {
    Enabled,
    Disabled,
    Adaptive,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Turn {
    pub query: Vec<LLMToken>,
    pub response: String,
}

impl Request {
    pub async fn bare_request(&self, log_dir: Option<String>) -> Result<Response, Error> {
        let logger = match log_dir {
            Some(log_dir) => Logger::new(log_dir, false, true),
            None => Logger::new(String::new(), false, false),
        };

        self.request_inner("", &logger).await
    }

    // VIBE NOTE: gemini 3.1 pro (via perplexity) taught me how to use `tokio::select` macro.
    pub async fn request(&self, working_dir: &str, logger: &Logger) -> Result<Response, Error> {
        let request_future = self.request_inner(working_dir, logger);
        tokio::pin!(request_future);
        let mut interval = tokio::time::interval(Duration::from_millis(200));

        loop {
            tokio::select! {
                result = &mut request_future => {
                    return result;
                },
                _ = interval.tick() => {
                    match check_interruption(working_dir) {
                        Ok(true) => {
                            return Err(Error::UserInterrupt);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                        _ => {},
                    }
                },
            }
        }
    }

    async fn request_inner(&self, working_dir: &str, logger: &Logger) -> Result<Response, Error> {
        let client = reqwest::Client::new();
        let mut error = None;

        for _ in 0..5 {
            let http_request = match self.model.provider() {
                ApiProvider::Anthropic => self.to_anthropic_request(working_dir)?,
                ApiProvider::OpenAi => self.to_openai_request(working_dir)?,
                ApiProvider::Mock => self.to_mock_request()?,
            };
            let mut api_log = ApiLog::new();
            let mut request = client
                .post(&http_request.url)
                .json(&http_request.body)
                .timeout(Duration::from_millis(600_000));

            for (key, value) in http_request.headers.iter() {
                request = request.header(key, value);
            }

            api_log.request_header = logger.log(LogEntry::SendRequest(request.try_clone().unwrap()))?;
            api_log.request_body = logger.log(LogEntry::RequestBody(http_request.body))?;

            // It has to generate all the logs that *real* API calls generate.
            if let ApiProvider::Mock = self.model.provider() {
                let mut response = self.send_mock_request(working_dir).await?;
                logger.log(LogEntry::GotResponse(200))?;
                api_log.response_header = logger.log(LogEntry::ResponseHeader(HashMap::new()))?;
                api_log.response_body = logger.log(LogEntry::ResponseText(serde_json::to_string_pretty(&response)?))?;
                logger.log_api_usage(response.cached_input_tokens, response.input_tokens, response.output_tokens)?;
                response.log = api_log;
                return Ok(response);
            }

            match request.send().await {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    logger.log(LogEntry::GotResponse(status_code))?;
                    api_log.response_header = logger.log(LogEntry::ResponseHeader(response.headers().iter().map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string())).collect()))?;

                    match response.text().await {
                        Ok(s) => {
                            api_log.response_body = logger.log(LogEntry::ResponseText(s.to_string()))?;

                            match status_code {
                                200..=299 => {
                                    let mut response = match self.model.provider() {
                                        ApiProvider::Anthropic => Response::from_anthropic(&s)?,
                                        ApiProvider::OpenAi => Response::from_openai(&s)?,
                                        ApiProvider::Mock => unreachable!(),
                                    };
                                    logger.log_api_usage(response.cached_input_tokens, response.input_tokens, response.output_tokens)?;
                                    response.log = api_log;
                                    return Ok(response);
                                },
                                429 => {
                                    logger.log(LogEntry::TooManyRequests)?;
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(60_000)).await;
                                },
                                500..600 => {
                                    logger.log(LogEntry::LLMServerBusy)?;
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(200_000)).await;
                                },
                                _ => {
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(20_000)).await;
                                },
                            }
                        },
                        Err(e) => {
                            logger.log(LogEntry::ReqwestError(format!("{e:?}")))?;
                            error = Some(Error::ReqwestError(e));
                            sleep(Duration::from_millis(20_000)).await;
                        },
                    }
                },
                Err(e) => {
                    logger.log(LogEntry::ReqwestError(format!("{e:?}")))?;
                    error = Some(Error::ReqwestError(e));
                    sleep(Duration::from_millis(20_000)).await;
                },
            }
        }

        Err(error.unwrap())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LLMToken {
    String(String),
    Image(ImageId),
}

pub fn count_bytes_of_llm_tokens(tokens: &[LLMToken], bytes_per_image: u64) -> u64 {
    tokens.iter().map(
        |token| match token {
            LLMToken::String(s) => s.len() as u64,
            LLMToken::Image(_) => bytes_per_image,
        }
    ).sum()
}

pub fn stringify_llm_tokens(tokens: &[LLMToken]) -> String {
    let mut ss = Vec::with_capacity(tokens.len());

    for token in tokens.iter() {
        match token {
            LLMToken::String(s) => {
                ss.push(s.to_string());
            },
            LLMToken::Image(id) => {
                ss.push(format!("Image {{{:016x}}}", id.0));
            },
        }
    }

    ss.join("\n")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiLog {
    pub request_header: Option<LogId>,
    pub request_body: Option<LogId>,
    pub response_header: Option<LogId>,
    pub response_body: Option<LogId>,
}

impl ApiLog {
    pub fn new() -> ApiLog {
        ApiLog {
            request_header: None,
            request_body: None,
            response_header: None,
            response_body: None,
        }
    }
}
