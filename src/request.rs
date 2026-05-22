use async_std::task::sleep;
use crate::{
    ApiProvider,
    Error,
    EtcModels,
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
mod openai_legacy;
mod gemini;

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

pub struct Config {
    pub request_timeout: u64,  // millis
    pub sleep_between_retry: u64,  // millis
    pub max_retry: usize,
    pub fallback_api_keys: HashMap<String, String>,
    pub etc_models: EtcModels,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            request_timeout: 600_000,
            sleep_between_retry: 300_000,
            max_retry: 4,
            fallback_api_keys: HashMap::new(),
            etc_models: EtcModels::default(),
        }
    }
}

impl Request {
    pub async fn bare_request(&self, config: &Config, log_dir: Option<String>) -> Result<Response, Error> {
        let logger = match log_dir {
            Some(log_dir) => Logger::new(log_dir, None, false, true),
            None => Logger::new(String::new(), None, false, false),
        };

        self.request_inner(config, "", &logger).await
    }

    // VIBE NOTE: gemini 3.1 pro (via perplexity) taught me how to use `tokio::select` macro.
    pub async fn request(&self, config: &Config, working_dir: &str, logger: &Logger) -> Result<Response, Error> {
        let request_future = self.request_inner(config, working_dir, logger);
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

    async fn request_inner(&self, config: &Config, working_dir: &str, logger: &Logger) -> Result<Response, Error> {
        let client = reqwest::Client::new();
        let mut error = None;

        for _ in 0..(config.max_retry + 1) {
            let http_request = match self.model.provider() {
                ApiProvider::Anthropic => self.to_anthropic_request(config, working_dir)?,
                ApiProvider::Openai => self.to_openai_request(config, working_dir)?,
                ApiProvider::OpenaiLegacy => self.to_openai_legacy_request(config, working_dir)?,
                ApiProvider::OpenaiImageEdit => unreachable!(),
                ApiProvider::Mock => self.to_mock_request(config)?,
                ApiProvider::Gemini => self.to_gemini_request(config, working_dir)?,
            };
            let mut api_log = ApiLog::new();
            let mut request = client
                .post(&http_request.url)
                .json(&http_request.body)
                .timeout(Duration::from_millis(config.request_timeout));

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
                logger.log_token_usage(response.cached_input_tokens, response.input_tokens, response.output_tokens)?;
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
                                        ApiProvider::Openai => Response::from_openai(&s)?,
                                        ApiProvider::OpenaiLegacy => Response::from_openai_legacy(&s)?,
                                        ApiProvider::OpenaiImageEdit => unreachable!(),
                                        ApiProvider::Mock => unreachable!(),
                                        ApiProvider::Gemini => Response::from_gemini(&s)?,
                                    };
                                    logger.log_token_usage(response.cached_input_tokens, response.input_tokens, response.output_tokens)?;
                                    response.log = api_log;
                                    return Ok(response);
                                },
                                429 => {
                                    logger.log(LogEntry::TooManyRequests)?;
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(config.sleep_between_retry)).await;
                                },
                                500..600 => {
                                    logger.log(LogEntry::LLMServerBusy)?;
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(config.sleep_between_retry)).await;
                                },
                                _ => {
                                    error = Some(Error::HttpError { status_code });
                                    sleep(Duration::from_millis(config.sleep_between_retry)).await;
                                },
                            }
                        },
                        Err(e) => {
                            logger.log(LogEntry::ReqwestError(format!("{e:?}")))?;
                            error = Some(Error::ReqwestError(e));
                            sleep(Duration::from_millis(config.sleep_between_retry)).await;
                        },
                    }
                },
                Err(e) => {
                    logger.log(LogEntry::ReqwestError(format!("{e:?}")))?;
                    error = Some(Error::ReqwestError(e));
                    sleep(Duration::from_millis(config.sleep_between_retry)).await;
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
