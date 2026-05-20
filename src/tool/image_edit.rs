// TODO: fancy stuffs in `src/request.rs` (e.g. api key fallback, api retry, logging...) are not implemented here

use crate::{Error, ImageId, encode_base64};
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageRequest {
    pub model: ImageModel,
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
    pub async fn request(&self, working_dir: &str) -> Result<ImageResponse, Error> {
        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(k) => k.to_string(),
            Err(_) => return Err(Error::ApiKeyNotFound { env_var: String::from("OPENAI_API_KEY") }),
        };
        let body = self.request_body(working_dir)?;
        let url = "https://api.openai.com/v1/images/edits";
        let client = reqwest::Client::new();
        let request = client.post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .timeout(Duration::from_millis(600_000));

        let response = request.send().await?.text().await?;
        let response: ImageResponse = serde_json::from_str(&response)?;
        eprintln!("{response:?}");
        todo!()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageResponse {
    pub data: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ImageModel {
    GptImage,
}

impl ImageModel {
    pub fn api_name(&self) -> &'static str {
        match self {
            ImageModel::GptImage => "gpt-image-2",
        }
    }
}
