//! MiniMax's picture API: one POST that answers with picture addresses (not an OpenAI images
//! body), paid per picture. The model and price come from the asset table.

use super::{AssetProvider, AssetRequest, Generated, Shape, SizeBy};
use autokill_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;

/// The provider id the asset table names.
pub const ID: &str = "minimax";
const BASE: &str = "https://api.minimaxi.com/v1";
const MAX_BYTES: u64 = 200 << 20;

/// The aspect ratio string MiniMax expects for each shape.
fn ratio(shape: Shape) -> &'static str {
    match shape {
        Shape::Square => "1:1",
        Shape::Wide => "16:9",
        Shape::Tall => "9:16",
    }
}

/// MiniMax, as an [`AssetProvider`].
pub struct MiniMax {
    base: String,
}

impl Default for MiniMax {
    fn default() -> MiniMax {
        MiniMax { base: BASE.to_string() }
    }
}

impl AssetProvider for MiniMax {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "MiniMax"
    }

    fn key_env(&self) -> Option<&'static str> {
        Some("MINIMAX_API_KEY")
    }

    fn keys_page(&self) -> Option<&'static str> {
        Some("https://platform.minimaxi.com/user-center/basic-information/interface-key")
    }

    fn generate(&self, key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
        let (model, prompt, shape) = match *req {
            AssetRequest::Image { model, prompt, shape } => (model, prompt, shape),
            AssetRequest::Model3d { .. } => {
                return Err(ToolError::new(ErrorCode::NotSupported, "MiniMax does not make 3D models.")
                    .with_remediation("Generate the picture here and the model with fal.ai."));
            }
        };
        let _ = SizeBy::ImageSize;
        let body = json!({"model": model.endpoint, "prompt": prompt, "aspect_ratio": ratio(shape), "response_format": "url", "n": 1});
        let url = format!("{}/image_generation", self.base.trim_end_matches('/'));
        let resp = agent(Duration::from_secs(5 * 60))
            .post(&url)
            .header("Authorization", &format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| {
                ToolError::new(ErrorCode::EngineMissing, format!("Couldn't reach MiniMax: {e}"))
                    .with_remediation("Check your internet connection and try again.")
            })?;
        let status = resp.status().as_u16();
        let text = resp.into_body().read_to_string().unwrap_or_default();
        match status {
            200..=299 => {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("MiniMax sent an unreadable answer: {e}")))?;
                let address = picture_url(&value).ok_or_else(|| {
                    ToolError::new(ErrorCode::GarbledOutput, "MiniMax didn't return a picture.")
                        .with_details(json!({"answer": value.to_string().chars().take(500).collect::<String>()}))
                })?;
                Ok(Generated { bytes: download(&address)?, url: address, cost_usd: req.listed_cost() })
            }
            401 | 403 => Err(ToolError::new(ErrorCode::BadSettings, "MiniMax didn't accept the key.")
                .with_remediation("Check the MiniMax key in Settings → AI → Game assets.")),
            402 => Err(ToolError::new(ErrorCode::OverLimit, "The MiniMax account is out of credit.")
                .with_remediation("Add credit in the MiniMax console, then try again.")),
            _ => Err(ToolError::new(ErrorCode::EngineMissing, format!("MiniMax answered {status}: {}", text.chars().take(300).collect::<String>()))),
        }
    }
}

/// The picture address: data.image_urls[0], or a base-64 picture inline.
fn picture_url(v: &Value) -> Option<String> {
    if let Some(url) = v.pointer("/data/image_urls/0").and_then(Value::as_str).filter(|u| !u.is_empty()) {
        return Some(url.to_string());
    }
    let b64 = v.pointer("/data/image_base64/0").and_then(Value::as_str)?;
    Some(format!("data:image/jpeg;base64,{b64}"))
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .user_agent("AutoKill")
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(timeout))
        .timeout_recv_body(Some(Duration::from_secs(10 * 60)))
        .build()
        .new_agent()
}

fn download(url: &str) -> Result<Vec<u8>, ToolError> {
    if let Some(data) = url.strip_prefix("data:") {
        let b64 = data
            .split_once(";base64,")
            .map(|(_, b)| b)
            .ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "MiniMax sent a picture in an unknown form."))?;
        return super::fal::base64_decode(b64);
    }
    let resp = agent(Duration::from_secs(120))
        .get(url)
        .call()
        .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("Couldn't download the picture: {e}")))?;
    if !resp.status().is_success() {
        return Err(ToolError::new(ErrorCode::EngineMissing, format!("Downloading the picture failed ({}).", resp.status().as_u16())));
    }
    let mut bytes = Vec::new();
    resp.into_body()
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("The download broke off: {e}")))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ToolError::new(ErrorCode::OverLimit, "The generated picture is too large."));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shape_gets_its_ratio() {
        assert_eq!(ratio(Shape::Square), "1:1");
        assert_eq!(ratio(Shape::Wide), "16:9");
        assert_eq!(ratio(Shape::Tall), "9:16");
    }

    #[test]
    fn a_url_or_inline_picture_both_count() {
        let url = json!({"data": {"image_urls": ["https://x/p.png"]}});
        assert_eq!(picture_url(&url).as_deref(), Some("https://x/p.png"));
        let inline = json!({"data": {"image_base64": ["AA=="]}});
        assert_eq!(picture_url(&inline).as_deref(), Some("data:image/jpeg;base64,AA=="));
        assert_eq!(picture_url(&json!({"data": {}})), None);
    }
}
