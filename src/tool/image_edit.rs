use crate::{Error, ImageId, Logger, LogEntry, Model, decode_base64, encode_base64};
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageRequest {
    pub model: Model,
    pub prompt: String,
    pub images: Vec<ImageId>,
    pub size: Option<(u64, u64)>,
}

impl ImageRequest {
    pub fn request_body(&self, working_dir: &str) -> Result<RequestBody, Error> {
        let mut images = Vec::with_capacity(self.images.len());

        for image in self.images.iter() {
            let bytes = read_bytes(&image.path(working_dir)?)?;
            let image_base64 = encode_base64(&bytes);
            images.push(RawImage {
                image_url: format!("data:image/png;base64,{image_base64}"),
            });
        }

        let size = match self.size {
            Some((w, h)) => format!("{w}x{h}"),
            None => String::from("auto"),
        };

        Ok(RequestBody {
            images,
            prompt: self.prompt.to_string(),
            model: self.model.api_name().to_string(),
            size,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestBody {
    pub images: Vec<RawImage>,
    pub prompt: String,
    pub model: String,
    pub size: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawImage {
    pub image_url: String,
}

impl ImageRequest {
    // It doesn't have fancy features (e.g. auto-retry) like `src/request.rs`.
    // My assumption is that, if there's an api error, neukgu will read the
    // error message and decide how to solve (or avoid) it.
    pub async fn request(&self, working_dir: &str, logger: &Logger) -> Result<ImageResponse, Error> {
        let api_key = match std::env::var(self.model.api_key_env_var()) {
            Ok(k) => k.to_string(),
            // TODO: fallback_api_keys
            Err(_) => return Err(Error::ApiKeyNotFound { env_var: String::from(self.model.api_key_env_var()) }),
        };
        let body = self.request_body(working_dir)?;
        let url = "https://api.openai.com/v1/images/edits";
        let client = reqwest::Client::new();
        let request = client.post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .timeout(Duration::from_millis(600_000));

        logger.log(LogEntry::SendRequest(request.try_clone().unwrap()))?;
        logger.log(LogEntry::RequestBody(serde_json::to_value(&body)?))?;

        let response = request.send().await?;
        let response_status = response.status().as_u16();
        logger.log(LogEntry::GotImageResponse(response_status))?;
        logger.log(LogEntry::ResponseHeader(response.headers().iter().map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string())).collect()))?;

        let response_text = response.text().await?;
        logger.log(LogEntry::ResponseText(response_text.to_string()))?;
        logger.log_image_edit_token_usage(/* TODO */)?;

        let response = match (response_status, response_text) {
            (200..=299, response) => response,
            (status_code @ 400.., response) => {
                return Err(Error::ImageRequestError {
                    status_code,
                    message: response,
                });
            },
            (status_code, _) => {
                return Err(Error::HttpError { status_code });
            },
        };
        let response: ImageResponse = serde_json::from_str(&response)?;
        Ok(response)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageResponse {
    pub data: Vec<ImageResponseData>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageResponseData {
    pub b64_json: String,
}

impl ImageResponseData {
    pub fn decode_base64(&self) -> Result<Vec<u8>, Error> {
        decode_base64(&self.b64_json)
    }
}
