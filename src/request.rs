use async_std::task::sleep;
use crate::{Error, ImageId, Logger, LogEntry, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

mod anthropic;

pub enum ApiProvider {
    Anthropic,
}

pub struct HttpRequest {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

pub struct Request {
    pub model: String,
    pub provider: ApiProvider,
    pub system_prompt: String,
    pub history: Vec<Turn>,
    pub query: Vec<StringOrImage>,
    pub enable_web_search: bool,
    pub thinking: Thinking,
}

pub enum Thinking {
    Enabled,
    Disabled,
    Adaptive,
}

pub struct Turn {
    pub query: Vec<StringOrImage>,
    pub response: String,
}

pub enum StringOrImage {
    String(String),
    Image(ImageId),
}

impl Request {
    pub async fn request(&mut self, logger: &mut Logger) -> Result<Response, Error> {
        let client = reqwest::Client::new();
        let mut error = None;

        for _ in 0..5 {
            let http_request = match self.provider {
                ApiProvider::Anthropic => self.to_anthropic_request()?,
            };
            let mut request = client
                .post(&http_request.url)
                .json(&http_request.body)
                .timeout(Duration::from_millis(600_000));

            for (key, value) in http_request.headers.iter() {
                request = request.header(key, value);
            }

            logger.log(LogEntry::SendRequest(request.try_clone().unwrap()))?;
            logger.log(LogEntry::RequestBody(http_request.body))?;

            match request.send().await {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    logger.log(LogEntry::GotResponse(status_code))?;
                    logger.log(LogEntry::ResponseHeader(response.headers().iter().map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string())).collect()))?;

                    match response.text().await {
                        Ok(s) => {
                            logger.log(LogEntry::ResponseText(s.to_string()))?;

                            match status_code {
                                200..=299 => {
                                    let response = match self.provider {
                                        // It's un-recoverable, so we just unwrap.
                                        ApiProvider::Anthropic => Response::from_anthropic(&s).unwrap(),
                                    };
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
