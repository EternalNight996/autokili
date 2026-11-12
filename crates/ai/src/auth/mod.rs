//! Sign-in that comes back to this app: OAuth 2.0 authorization code + PKCE, the browser page
//! opened for the person, the answer received on a loopback port, and the grant written where the
//! agent reads it. Nothing is typed into a terminal and no key ever passes through the clipboard.

pub mod deepseek;

use autokill_evidence::{ErrorCode, ToolError};
use autokill_platform::process::Process;
use std::time::Duration;

/// Opens `url` in the person's browser and returns at once: the page is theirs to finish, and the
/// loopback listener waits for it. A program that cannot be started is an error with a fix.
pub fn open_in_browser(url: &str) -> Result<(), ToolError> {
    let (program, args): (&str, [&str; 2]) = if cfg!(windows) {
        ("cmd", ["/c", "start"])
    } else if cfg!(target_os = "macos") {
        ("open", ["", ""])
    } else {
        ("xdg-open", ["", ""])
    };
    let mut process = Process::new(program).hands_off().timeout(Duration::from_secs(20));
    process = process.arg(args[0]);
    if !args[1].is_empty() {
        process = process.arg(args[1]);
    }
    process = process.arg(url);
    let mut running = process.spawn()?;
    let _ = running.wait_timeout(Duration::from_secs(20));
    Ok(())
}

/// A message for the person when the browser could not be opened: the address stays usable.
pub fn browser_failed(url: &str, e: ToolError) -> ToolError {
    ToolError::new(ErrorCode::ProgramFailed, format!("Could not open the sign-in page: {}", e.message))
        .with_remediation(format!("Open this address in your browser, then finish signing in: {url}"))
}
