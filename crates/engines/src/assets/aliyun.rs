//! Alibaba Cloud Bailian's picture API (Tongyi Wanxiang): asynchronous — one POST creates a
//! task, then the task is polled until it succeeds and names the picture. Prices come from the
//! asset table.

use super::{AssetProvider, AssetRequest, Generated, Shape, SizeBy};
use autokill_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::io::Read;
use std::time::{Duration, Instant};

/// The provider id the asset table names.
pub const ID: &str = "aliyun";
const BASE: &str = "https://dashscope.aliyuncs.com/api/v1";
const MAX_BYTES: u64 = 200 << 20;
/// A picture takes a few seconds; a hung task must not hold the build.
const TASK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const POLL_EVERY: Duration = Duration::from_secs(2);

/// The size Wanxiang expects: pixels joined by a star, not an x.
fn wanx_size(shape: Shape) -> &'static str {
    match shape {
        Shape::Square => "1024*1024",
        Shape::Wide => "1440*810",
        Shape::Tall => "810*1440",
    }
}

/// Bailian's Tongyi Wanxiang, as an [`AssetProvider`].
pub struct Aliyun {
    base: String,
}

impl Default for Aliyun {
    fn default() -> Aliyun {
        Aliyun { base: BASE.to_string() }
    }
}

impl AssetProvider for Aliyun {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "阿里云百炼 通义万相"
    }

    fn key_env(&self) -> Option<&'static str> {
        Some("DASHSCOPE_API_KEY")
    }

    fn keys_page(&self) -> Option<&'static str> {
        Some("https://bailian.console.aliyun.com/?apiKey=1")
    }

    fn generate(&self, key: &str, req: &AssetRequest) -> Result<Generated, ToolError> {
        let (model, prompt, shape) = match *req {
            AssetRequest::Image { model, prompt, shape } => (model, prompt, shape),
            AssetRequest::Model3d { .. } => {
                return Err(ToolError::new(ErrorCode::NotSupported, "Wanxiang does not make 3D models.")
                    .with_remediation("Generate the picture here and the model with fal.ai."));
            }
        };
        let _ = SizeBy::ImageSize;
        let body = json!({
            "model": model.endpoint,
            "input": {"prompt": prompt},
            "parameters": {"size": wanx_size(shape), "n": 1}
        });
        let url = format!("{}/services/aigc/text2image/image-synthesis", self.base.trim_end_matches('/'));
        let created = post(key, &url, &body)?;
        let task = created
            .pointer("/output/task_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "Wanxiang didn't return a task id."))?
            .to_string();
        let address = self.wait(key, &task)?;
        Ok(Generated { bytes: download(&address)?, url: address, cost_usd: req.listed_cost() })
    }
}

impl Aliyun {
    /// Poll the task until it names a picture, fails, or the deadline passes.
    fn wait(&self, key: &str, task: &str) -> Result<String, ToolError> {
        let url = format!("{}/tasks/{task}", self.base.trim_end_matches('/'));
        let deadline = Instant::now() + TASK_TIMEOUT;
        loop {
            let v = get(key, &url)?;
            let status = v.pointer("/output/task_status").and_then(Value::as_str).unwrap_or_default();
            match status {
                "SUCCEEDED" => {
                    return v
                        .pointer("/output/results/0/url")
                        .and_then(Value::as_str)
                        .filter(|u| !u.is_empty())
                        .map(str::to_string)
                        .ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "Wanxiang said it succeeded without a picture."));
                }
                "FAILED" | "CANCELED" => {
                    let why = v.pointer("/output/message").and_then(Value::as_str).unwrap_or("no reason given");
                    return Err(ToolError::new(ErrorCode::ProgramFailed, format!("Wanxiang refused that picture: {why}"))
                        .with_remediation("Describe it differently, then try again."));
                }
                _ => {
                    if Instant::now() >= deadline {
                        return Err(ToolError::new(ErrorCode::TimedOut, "Wanxiang didn't finish the picture in five minutes.")
                            .with_remediation("Try again, or pick another picture model in Settings → AI → Game assets."));
                    }
                    std::thread::sleep(POLL_EVERY);
                }
            }
        }
    }
}

fn post(key: &str, url: &str, body: &Value) -> Result<Value, ToolError> {
    let resp = agent(Duration::from_secs(60))
        .post(url)
        .header("Authorization", &format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .header("X-DashScope-Async", "enable")
        .send(body.to_string())
        .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("Couldn't reach 阿里云百炼: {e}"))
            .with_remediation("Check your internet connection and try again."))?;
    read(resp, "阿里云百炼")
}

fn get(key: &str, url: &str) -> Result<Value, ToolError> {
    let resp = agent(Duration::from_secs(30))
        .get(url)
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("Couldn't reach 阿里云百炼: {e}")))?;
    read(resp, "阿里云百炼")
}

fn read(resp: ureq::http::Response<ureq::Body>, who: &str) -> Result<Value, ToolError> {
    let status = resp.status().as_u16();
    let text = resp.into_body().read_to_string().unwrap_or_default();
    match status {
        200..=299 => serde_json::from_str(&text)
            .map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("{who} sent an unreadable answer: {e}"))),
        401 | 403 => Err(ToolError::new(ErrorCode::BadSettings, format!("{who} didn't accept the key."))
            .with_remediation("Check the DashScope key in Settings → AI → Game assets.")),
        402 => Err(ToolError::new(ErrorCode::OverLimit, format!("The {who} account is out of credit."))
            .with_remediation("Add credit in the Bailian console, then try again.")),
        _ => Err(ToolError::new(ErrorCode::EngineMissing, format!("{who} answered {status}: {}", text.chars().take(300).collect::<String>()))),
    }
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
    fn shapes_are_asked_for_in_star_sizes() {
        assert_eq!(wanx_size(Shape::Square), "1024*1024");
        assert_eq!(wanx_size(Shape::Wide), "1440*810");
        assert_eq!(wanx_size(Shape::Tall), "810*1440");
    }
}
