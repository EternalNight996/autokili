//! DeepSeek Platform account sign-in for DeepSeek Harness (the same flow the Harness desktop app
//! uses): the Platform authorises this app with PKCE, the code comes back to a loopback port, and
//! the grant goes into the Harness credential store so `dsh --profile headless` answers.

use autokill_evidence::{ErrorCode, ToolError};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

/// Where the account grant lives in the Harness credential store.
pub const ACCOUNT_KEY: &str = "deepseek-account-platform/default";
/// The stable device id the Platform attaches to this installation.
pub const DEVICE_KEY: &str = "deepseek-account-platform/device";

const DEFAULT_ORIGIN: &str = "https://platform.deepseek.com";
const CALLBACK_PATH: &str = "/oauth/callback";
const LIFETIME: Duration = Duration::from_secs(15 * 60);

fn origin() -> String {
    std::env::var("AUTOKILL_DEEPSEEK_PLATFORM_ORIGIN")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string())
}

/// `$DSH_HOME/.credentials.yaml`, else `~/.dsh/.credentials.yaml`.
pub fn credentials_file() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("DSH_HOME") {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home).join(".credentials.yaml"));
        }
    }
    dirs::home_dir().map(|h| h.join(".dsh").join(".credentials.yaml"))
}

fn read_store(path: &std::path::Path) -> yaml_rust2::Yaml {
    use yaml_rust2::YamlLoader;
    let text = std::fs::read_to_string(path).unwrap_or_default();
    if text.trim().is_empty() {
        let mut map = yaml_rust2::yaml::Hash::new();
        map.insert(yaml_rust2::Yaml::from_str("version"), yaml_rust2::Yaml::Integer(1));
        map.insert(yaml_rust2::Yaml::from_str("refs"), yaml_rust2::Yaml::Hash(Default::default()));
        map.insert(yaml_rust2::Yaml::from_str("records"), yaml_rust2::Yaml::Hash(Default::default()));
        return yaml_rust2::Yaml::Hash(map);
    }
    YamlLoader::load_from_str(&text)
        .ok()
        .and_then(|mut docs| docs.drain(..).next())
        .filter(|y| matches!(y, yaml_rust2::Yaml::Hash(_)))
        .unwrap_or(yaml_rust2::Yaml::Hash(Default::default()))
}

fn write_store(path: &std::path::Path, store: &yaml_rust2::Yaml) -> Result<(), ToolError> {
    use yaml_rust2::YamlEmitter;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            ToolError::new(ErrorCode::FileError, format!("Could not create {}: {e}", dir.display()))
        })?;
    }
    let mut out = String::new();
    let mut emitter = YamlEmitter::new(&mut out);
    emitter.dump(store).map_err(|e| ToolError::new(ErrorCode::FileError, format!("Could not write the credential store: {e}")))?;
    out.push('\n');
    let tmp = path.with_extension("yaml.tmp");
    std::fs::write(&tmp, out.as_bytes()).map_err(|e| {
        ToolError::new(ErrorCode::FileError, format!("Could not write {}: {e}", tmp.display()))
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        ToolError::new(ErrorCode::FileError, format!("Could not replace {}: {e}", path.display()))
    })
}

/// The token the Harness holds for the DeepSeek Platform account, when one is stored.
pub fn stored_token() -> Option<String> {
    stored_token_at(&credentials_file()?)
}

fn stored_token_at(path: &std::path::Path) -> Option<String> {
    let store = read_store(path);
    let records = store["records"].as_hash()?;
    let record = records.get(&yaml_rust2::Yaml::from_str(ACCOUNT_KEY))?;
    let payload = record["payload"].as_hash()?;
    payload.get(&yaml_rust2::Yaml::from_str("token"))?.as_str().map(str::to_string)
}

/// Whether the Harness already has an account grant, i.e. a build will not stop on a missing key.
pub fn has_grant() -> bool {
    stored_token().is_some_and(|t| !t.trim().is_empty())
}

fn device_id() -> Result<String, ToolError> {
    let path = credentials_file().ok_or_else(|| {
        ToolError::new(ErrorCode::FileError, "No home folder to keep the Harness credentials in.").with_remediation("Set DSH_HOME to a folder you own and try again.")
    })?;
    device_id_at(&path)
}

fn device_id_at(path: &std::path::Path) -> Result<String, ToolError> {
    use yaml_rust2::yaml::Hash;
    let mut store = read_store(path);
    let existing = store["records"].as_hash().and_then(|records| {
        records.get(&yaml_rust2::Yaml::from_str(DEVICE_KEY))?["payload"].as_hash()?.get(&yaml_rust2::Yaml::from_str("id"))?.as_str().map(str::to_string)
    });
    if let Some(id) = existing.filter(|id| id.len() >= 32) {
        return Ok(id);
    }
    let id = new_uuid();
    if let yaml_rust2::Yaml::Hash(root) = &mut store {
        let records = root
            .entry(yaml_rust2::Yaml::from_str("records"))
            .or_insert_with(|| yaml_rust2::Yaml::Hash(Hash::new()));
        if !matches!(records, yaml_rust2::Yaml::Hash(_)) {
            *records = yaml_rust2::Yaml::Hash(Hash::new());
        }
        if let yaml_rust2::Yaml::Hash(records) = records {
            let mut payload = Hash::new();
            payload.insert(yaml_rust2::Yaml::from_str("id"), yaml_rust2::Yaml::String(id.clone()));
            let mut record = Hash::new();
            record.insert(yaml_rust2::Yaml::from_str("kind"), yaml_rust2::Yaml::String("grant".into()));
            record.insert(yaml_rust2::Yaml::from_str("payload"), yaml_rust2::Yaml::Hash(payload));
            records.insert(yaml_rust2::Yaml::from_str(DEVICE_KEY), yaml_rust2::Yaml::Hash(record));
        }
    }
    write_store(path, &store)?;
    Ok(id)
}

/// A RFC 4122-shaped id, made from this process's random hasher: no extra dependency for one id.
fn new_uuid() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let v = h.finish().to_le_bytes();
        let n = chunk.len().min(8);
        chunk[..n].copy_from_slice(&v[..n]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

fn auth_headers() -> Vec<(&'static str, String)> {
    vec![
        ("x-client-bundle-id", String::new()),
        ("x-client-platform", if cfg!(windows) { "desktop-win".into() } else if cfg!(target_os = "macos") { "desktop-mac".into() } else { "web".into() }),
        ("x-client-version", env!("CARGO_PKG_VERSION").to_string()),
        ("x-client-locale", "zh_CN".into()),
        ("x-client-timezone-offset", "28800".into()),
    ]
}

fn envelope(body: &serde_json::Value) -> Result<serde_json::Value, ToolError> {
    let ok = body["code"].as_i64() == Some(0) && body["data"]["biz_code"].as_i64() == Some(0);
    if !ok {
        return Err(ToolError::new(ErrorCode::ProgramFailed, "DeepSeek Platform 拒绝了这次登录。").with_remediation("重新发起登录；若仍失败，先在 platform.deepseek.com 登录一次再试。"));
    }
    Ok(body["data"]["biz_data"].clone())
}

/// Runs the whole sign-in and returns the line shown to the person. Blocks until the browser comes
/// back, the flow expires, or the Platform refuses it: call it off the UI thread.
pub fn sign_in() -> Result<String, ToolError> {
    let origin = origin();
    let agent = crate::http::quick_agent(Duration::from_secs(25));
    let verifier = token(32);
    let state = token(32);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| {
        ToolError::new(ErrorCode::EngineMissing, format!("Could not open a local callback port: {e}")).with_remediation("Check that a firewall is not blocking loopback connections, then try again.")
    })?;
    let port = listener.local_addr().map_err(|e| ToolError::new(ErrorCode::EngineMissing, e.to_string()))?.port();
    let redirect = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    listener.set_nonblocking(false).ok();

    let init = serde_json::json!({
        "code_challenge": challenge,
        "code_challenge_method": "S256",
        "state": state,
        "redirect_uri": redirect,
        "locale": "zh_CN",
        "login_source": "desktop",
    });
    let url = format!("{origin}/auth-api/v0/dsh/auth_init");
    let mut reader = crate::http::post_json(&agent, "DeepSeek Platform", &url, &auth_headers(), &init)?;
    let mut text = String::new();
    use std::io::Read as _;
    reader.read_to_string(&mut text).ok();
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("DeepSeek Platform answered something unreadable: {e}")))?;
    let data = envelope(&body)?;
    let authorize_url = data["authorize_url"].as_str().unwrap_or_default().to_string();
    if !authorize_url.starts_with(&format!("{origin}/")) {
        return Err(ToolError::new(ErrorCode::GarbledOutput, "DeepSeek Platform returned an unexpected sign-in address."));
    }

    super::open_in_browser(&authorize_url).map_err(|e| super::browser_failed(&authorize_url, e))?;

    // One callback, then the port closes. The listener times out with the flow.
    let (mut stream, _) = {
        listener.set_nonblocking(true).ok();
        let deadline = std::time::Instant::now() + LIFETIME;
        loop {
            match listener.accept() {
                Ok(pair) => break pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return Err(ToolError::new(ErrorCode::TimedOut, "DeepSeek 登录超时。").with_remediation("点「授权登录」再来一次。"));
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
                Err(e) => return Err(ToolError::new(ErrorCode::ProgramFailed, format!("Callback port failed: {e}"))),
            }
        }
    };
    stream.set_nonblocking(false).ok();
    stream.set_read_timeout(Some(Duration::from_secs(20))).ok();

    let mut request_line = String::new();
    {
        let mut reader = BufReader::new(&stream);
        reader.read_line(&mut request_line).ok();
    }
    let target = request_line.split_whitespace().nth(1).unwrap_or_default().to_string();
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or_default().to_string();
    let params: Vec<(String, String)> = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let get = |name: &str| params.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());

    let reply = |stream: &mut std::net::TcpStream, title: &str, detail: &str| {
        let body = format!(
            "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><body style=\"font:14px/1.6 system-ui,sans-serif;display:grid;place-items:center;min-height:100vh;background:#101014;color:#eef\"><main style=\"text-align:center\"><h1 style=\"font-size:18px\">{title}</h1><p style=\"color:#9aa\">{detail}</p><script>setTimeout(() => window.close(), 1500)</script></main></body>"
        );
        let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body);
        let _ = stream.flush();
    };

    if target.split('?').next() != Some(CALLBACK_PATH) {
        reply(&mut stream, "登录失败", "回调地址不对，请回到 AutoKill 重新发起。");
        return Err(ToolError::new(ErrorCode::BadInput, "The browser came back on an unexpected address."));
    }
    let (Some(code), Some(back_state)) = (get("code"), get("state")) else {
        reply(&mut stream, "登录失败", "回调缺少参数，请回到 AutoKill 重新发起。");
        return Err(ToolError::new(ErrorCode::BadInput, "The sign-in callback carried no code."));
    };
    if back_state != state {
        reply(&mut stream, "登录失败", "本次登录校验不通过，请重新发起。");
        return Err(ToolError::new(ErrorCode::BadInput, "The sign-in callback did not match this attempt."));
    }

    let device = match device_id() {
        Ok(d) => d,
        Err(e) => {
            reply(&mut stream, "登录失败", "无法写入 DeepSeek Harness 的凭据文件。");
            return Err(e);
        }
    };
    let exchange = serde_json::json!({
        "code": code,
        "code_verifier": verifier,
        "redirect_uri": redirect,
        "device_id": device,
        "device_model": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "os_version": std::env::consts::OS,
    });
    let url = format!("{origin}/auth-api/v0/dsh/auth_exchange");
    let exchanged = crate::http::post_json(&agent, "DeepSeek Platform", &url, &auth_headers(), &exchange)
        .and_then(|mut r| {
            let mut text = String::new();
            r.read_to_string(&mut text).ok();
            serde_json::from_str::<serde_json::Value>(&text).map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("DeepSeek Platform answered something unreadable: {e}")))
        })
        .and_then(|body| envelope(&body))
        .and_then(|data| {
            data["token"].as_str().filter(|t| !t.is_empty()).map(str::to_string).ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "DeepSeek Platform returned no token."))
        });
    let token = match exchanged {
        Ok(t) => t,
        Err(e) => {
            reply(&mut stream, "登录失败", "平台没有返回凭据，请回到 AutoKill 重试。");
            return Err(e);
        }
    };
    if let Err(e) = store_grant(&token, &origin) {
        reply(&mut stream, "登录失败", "凭据写入失败，请检查 DeepSeek Harness 的凭据文件权限。");
        return Err(e);
    }
    reply(&mut stream, "已授权", "DeepSeek 账号已登录，可以关闭此页面回到 AutoKill。");
    Ok("已授权：DeepSeek Platform（DeepSeek Harness 可以直接构建）".to_string())
}

fn token(bytes: usize) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ (out.len() as u128));
        out.extend_from_slice(&h.finish().to_le_bytes());
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&out[..bytes])
}

/// Saves the grant in the format the Harness credential store reads, keeping every other entry.
pub fn store_grant(token: &str, issuer: &str) -> Result<(), ToolError> {
    let path = credentials_file().ok_or_else(|| ToolError::new(ErrorCode::FileError, "No home folder for the Harness credentials."))?;
    store_grant_at(&path, token, issuer)
}

fn store_grant_at(path: &std::path::Path, token: &str, issuer: &str) -> Result<(), ToolError> {
    use yaml_rust2::yaml::Hash;
    let mut store = read_store(path);
    if let yaml_rust2::Yaml::Hash(root) = &mut store {
        root.entry(yaml_rust2::Yaml::from_str("version")).or_insert(yaml_rust2::Yaml::Integer(1));
        let records = root.entry(yaml_rust2::Yaml::from_str("records")).or_insert_with(|| yaml_rust2::Yaml::Hash(Hash::new()));
        if !matches!(records, yaml_rust2::Yaml::Hash(_)) {
            *records = yaml_rust2::Yaml::Hash(Hash::new());
        }
        if let yaml_rust2::Yaml::Hash(records) = records {
            let mut payload = Hash::new();
            payload.insert(yaml_rust2::Yaml::from_str("version"), yaml_rust2::Yaml::Integer(1));
            payload.insert(yaml_rust2::Yaml::from_str("token"), yaml_rust2::Yaml::String(token.to_string()));
            payload.insert(yaml_rust2::Yaml::from_str("issuer"), yaml_rust2::Yaml::String(issuer.to_string()));
            let mut record = Hash::new();
            record.insert(yaml_rust2::Yaml::from_str("kind"), yaml_rust2::Yaml::String("grant".into()));
            record.insert(yaml_rust2::Yaml::from_str("payload"), yaml_rust2::Yaml::Hash(payload));
            records.insert(yaml_rust2::Yaml::from_str(ACCOUNT_KEY), yaml_rust2::Yaml::Hash(record));
        }
    }
    write_store(path, &store)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".credentials.yaml");
        (dir, path)
    }

    #[test]
    fn a_grant_is_written_where_the_harness_reads_it_and_nothing_else_is_lost() {
        let (_dir, path) = store();
        std::fs::write(&path, "version: 1\nrefs:\n  other: value\nrecords:\n  keep:me:\n    kind: grant\n    payload:\n      token: abc\n").unwrap();
        assert_eq!(stored_token_at(&path), None);
        store_grant_at(&path, "tok-123", "https://platform.deepseek.com").unwrap();
        assert_eq!(stored_token_at(&path).as_deref(), Some("tok-123"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("keep:me"), "the other record stays: {text}");
        assert!(text.contains("tok-123"), "{text}");
    }

    #[test]
    fn a_device_id_is_made_once_and_reused() {
        let (_dir, path) = store();
        let first = device_id_at(&path).unwrap();
        let second = device_id_at(&path).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 36);
    }

    #[test]
    fn tokens_are_url_safe_and_long_enough() {
        let t = token(32);
        assert_eq!(t.len(), 43);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
