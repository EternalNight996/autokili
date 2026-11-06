//! Kimi Code CLI's own Kimi account: `kimi login` signs in (the CLI opens the browser) and
//! `kimi logout` signs out. Craft Anything only reads whether that account exists and runs the
//! CLI's own commands, so the sign-in and the credentials stay Kimi's.

use craft_anything_evidence::{ErrorCode, ToolError};
use craft_anything_platform::process::Process;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The environment variable that moves Kimi Code CLI's folder (`~/.kimi` by default).
pub const SHARE_VAR: &str = "KIMI_SHARE_DIR";
/// How long the CLI's own sign-in waits for the browser.
pub const SIGN_IN_WAIT: Duration = Duration::from_secs(15 * 60);
/// A sign-out is quick; it only clears what's stored.
const SIGN_OUT_WAIT: Duration = Duration::from_secs(60);
/// The most of the CLI's output kept.
const MAX_OUTPUT: usize = 256 * 1024;

/// The folder Kimi Code CLI keeps its configuration and credentials in.
pub fn home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(SHARE_VAR).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let usual = dirs::home_dir()?;
    let current = usual.join(".kimi");
    if current.is_dir() {
        return Some(current);
    }
    // Kimi Code's app folder, for an install that keeps its data there.
    let app = usual.join(".kimi-code");
    Some(if app.is_dir() { app } else { current })
}

/// Whether the CLI is signed in: a credential file it wrote (`credentials/*.json`).
pub fn signed_in(home: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(home.join("credentials")) else { return false };
    entries.flatten().take(64).any(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
}

/// The `kimi` program to run: the one the person chose for this backend, else the one on PATH.
pub fn find_program(chosen: Option<&str>) -> Result<String, ToolError> {
    if let Some(chosen) = chosen.map(str::trim).filter(|c| !c.is_empty()) {
        return Ok(chosen.to_string());
    }
    craft_anything_platform::which("kimi").map(|p| p.display().to_string()).ok_or_else(|| {
        ToolError::new(ErrorCode::EngineMissing, "Kimi Code CLI isn't installed, or isn't on your PATH.")
            .with_remediation("Install Kimi Code CLI (irm https://code.kimi.com/kimi-code/install.ps1 | iex), or set its program in Settings → AI.")
    })
}

/// Sign in by running the CLI's own `kimi login`, which opens the browser. Blocking, with a
/// deadline: call it off the UI thread.
pub fn sign_in(program: &str) -> Result<String, ToolError> {
    run(program, "login", SIGN_IN_WAIT)
}

/// Sign out by running the CLI's own `kimi logout`.
pub fn sign_out(program: &str) -> Result<String, ToolError> {
    run(program, "logout", SIGN_OUT_WAIT)
}

/// Run one of the CLI's own account commands and give back the last of what it said.
fn run(program: &str, command: &str, wait: Duration) -> Result<String, ToolError> {
    let out = Process::new(program)
        .arg(command)
        .timeout(wait)
        .stdout_cap(MAX_OUTPUT)
        .stderr_cap(MAX_OUTPUT)
        .run()
        .map_err(|e| ToolError::new(ErrorCode::EngineMissing, format!("Couldn't run {program} {command}: {e}")))?;
    if !out.success {
        let said = tail(out.stderr_text().trim());
        let said = if said.is_empty() { tail(out.stdout_text().trim()) } else { said };
        return Err(ToolError::new(
            ErrorCode::EngineMissing,
            format!("{program} {command} failed{}.", if said.is_empty() { String::new() } else { format!(": {said}") }),
        )
        .with_remediation("Run it in a terminal to see what it asks for, then try again."));
    }
    Ok(tail(out.stdout_text().trim()))
}

/// The last lines of what a command said, bounded for a message.
fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(6);
    let joined = lines[start..].join(" ");
    joined.chars().take(600).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_file_is_a_sign_in() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!signed_in(dir.path()), "no credentials folder");
        std::fs::create_dir_all(dir.path().join("credentials")).unwrap();
        assert!(!signed_in(dir.path()), "an empty folder isn't a sign-in");
        std::fs::write(dir.path().join("credentials/kimi-code.json"), b"{}").unwrap();
        assert!(signed_in(dir.path()));
    }

    #[test]
    fn a_missing_program_is_named_with_a_fix() {
        let err = find_program(Some("  ")).unwrap_err();
        assert!(err.remediation.is_some(), "{err:?}");
        assert_eq!(find_program(Some("C:/tools/kimi.exe")).unwrap(), "C:/tools/kimi.exe");
    }

    #[test]
    fn the_last_lines_of_an_answer_are_kept() {
        assert_eq!(tail("one\n\ntwo\nthree\n"), "one two three");
        assert!(tail(&"x".repeat(2000)).chars().count() <= 600);
    }
}
