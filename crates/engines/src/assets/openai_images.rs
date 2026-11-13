//! Pictures from the domestic OpenAI-compatible image APIs (Zhipu CogView, Volcengine Ark's
//! Seedream, SiliconFlow): one POST to {base}/images/generations with the model, the prompt and
//! a size, answered with a picture address. Prices come from the asset table.

use super::{AssetProvider, AssetRequest, Generated, Shape};
use autokill_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;

/// Largest download we keep.
const MAX_BYTES: u64 = 200 << 20;

/// One vendor that speaks the OpenAI images shape.
#[derive(Clone, Copy)]
pub struct Vendor {
    /// The provider id the asset table names.
    pub id: &'static str,
    /// Shown to the person.
    pub name: &'static str,
    /// The API root, without the images path.
    pub base: &'static str,
    /// The environment variable its console shows.
    pub key_env: &'static str,
    /// Where the person makes a key.
    pub keys_page: &'static str,
}

/// The vendors AutoKill can talk to without an adapter of their own.
pub const VENDORS: &[Vendor] = &[
    Vendor {
        id: "zhipu",
        name: "智谱 CogView",
        base: "https://open.bigmodel.cn/api/paas/v4",
        key_env: "ZHIPUAI_API_KEY",
        keys_page: "https://open.bigmodel.cn/usercenter/apikeys",
    },
    Vendor {
        id: "volcengine",
        name: "火山方舟 Seedream",
        base: "https://ark.cn-beijing.volces.com/api/v3",
        key_env: "ARK_API_KEY",
        keys_page: "https://console.volcengine.com/ark",
    },
    Vendor {
        id: "siliconflow",
        name: "硅基流动 SiliconFlow",
        base: "https://api.siliconflow.cn/v1",
        key_env: "SILICONFLOW_API_KEY",
        keys_page: "https://cloud.siliconflow.cn/account/ak",
    },
];

/// The size each shape is asked for: these APIs take pixels, not names.
fn pixels(shape: Shape) -> (&'static str, &'static str) {
    match shape {
        Shape::Square => ("size", "1024x1024"),
        Shape::Wide => ("size", "1536x1024"),
        Shape::Tall => ("size", "1024x1536"),
    }
}

/// One of VENDORS, as an AssetProvider.
pub struct OpenAiImages {
    pub vendor: Vendor,
    pub base: String,
}

impl OpenAiImages {
    pub fn new(vendor: Vendor) -> Self {
        OpenAiImages { vendor, base: vendor.base.to_string() }
    }
}

impl AssetProvider for OpenAiImages {
    fn id(&self) -> &'static str {
        self.vendor.id
    }

    fn name(&self) -> &'static str {
        self.vendor.name
    }

    fn keys_page(&self) -> Option<&'static str> {
        Some(self.vendor.keys_page)
    }

    fn key_env(&self) -> Option<&'static str> {
        Some(self.vendor.key_env)
    }

    fn generate(&self, key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
        let (model, prompt, shape) = match *req {
            AssetRequest::Image { model, prompt, shape } => (model, prompt, shape),
            AssetRequest::Model3d { .. } => {
                return Err(ToolError::new(ErrorCode::NotSupported, format!("{} does not make 3D models.", self.name()))
                    .with_remediation("Generate the picture with this provider and the model with fal.ai."));
            }
        };
        let (field, size) = pixels(shape);
        let body = json!({"model": model.endpoint, "prompt": prompt, field: size, "n": 1, "response_format": "url"});
        let url = format!("{}/images/generations", self.base.trim_end_matches('/'));
        let resp = agent(Duration::from_secs(5 * 60))
            .post(&url)
            .header("Authorization", &format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| {
                ToolError::new(ErrorCode::EngineMissing, format!("Couldn't reach {}: {e}", self.name()))
                    .with_remediation("Check your internet connection and try again.")
            })?;
        let status = resp.status().as_u16();
        let text = resp.into_body().read_to_string().unwrap_or_default();
        match status {
            200..=299 => {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("{} sent an unreadable answer: {e}", self.name())))?;
                let address = picture_url(&value).ok_or_else(|| {
                    ToolError::new(ErrorCode::GarbledOutput, format!("{} didn't return a picture.", self.name()))
                        .with_details(json!({"answer": value.to_string().chars().take(500).collect::<String>()}))
                })?;
                Ok(Generated { bytes: download(&address)?, url: address, cost_usd: req.listed_cost() })
            }
            401 | 403 => Err(ToolError::new(ErrorCode::BadSettings, format!("{} didn't accept the key.", self.name()))
                .with_remediation(format!("Check the {} key in Settings → AI → Game assets.", self.name()))),
            402 => Err(ToolError::new(ErrorCode::OverLimit, format!("The {} account is out of credit.", self.name()))
                .with_remediation("Add credit in that console, then try again.")),
            _ => Err(ToolError::new(ErrorCode::EngineMissing, format!("{} answered {status}: {}", self.name(), text.chars().take(300).collect::<String>()))),
        }
    }
}

/// The picture address: data[0].url, or the base-64 an API may inline instead.
fn picture_url(v: &Value) -> Option<String> {
    if let Some(url) = v.pointer("/data/0/url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
        return Some(url.to_string());
    }
    let b64 = v.pointer("/data/0/b64_json").and_then(Value::as_str)?;
    Some(format!("data:image/png;base64,{b64}"))
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

/// Fetch the picture the API named, or decode a data: URI.
fn download(url: &str) -> Result<Vec<u8>, ToolError> {
    if let Some(data) = url.strip_prefix("data:") {
        let b64 = data
            .split_once(";base64,")
            .map(|(_, b)| b)
            .ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "The provider sent a file in an unknown form."))?;
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
    fn every_shape_is_asked_for_in_pixels() {
        for shape in [Shape::Square, Shape::Wide, Shape::Tall] {
            let (field, size) = pixels(shape);
            assert_eq!(field, "size");
            assert!(size.contains('x'), "{shape:?} -> {size}");
        }
    }

    #[test]
    fn a_picture_address_or_inline_base64_both_count() {
        let url = json!({"data": [{"url": "https://x/p.png"}]});
        assert_eq!(picture_url(&url).as_deref(), Some("https://x/p.png"));
        let inline = json!({"data": [{"b64_json": "AA=="}]});
        assert_eq!(picture_url(&inline).as_deref(), Some("data:image/png;base64,AA=="));
        assert_eq!(picture_url(&json!({"data": []})), None);
    }

    #[test]
    fn every_vendor_names_its_key_and_page() {
        for v in VENDORS {
            assert!(!v.id.is_empty() && !v.base.is_empty());
            assert!(v.key_env.ends_with("_API_KEY"), "{}", v.key_env);
            assert!(v.keys_page.starts_with("https://"));
        }
    }
}
