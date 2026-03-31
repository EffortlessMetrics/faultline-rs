use std::process::Command;

/// Build the error message for a missing tool.
/// This is separated from `ensure_tool` so it can be tested without calling `process::exit`.
pub fn missing_tool_message(name: &str, install_cmd: &str) -> String {
    format!("error: {name} is not installed\n  install: {install_cmd}")
}

/// Check that a tool binary is available on PATH.
/// If missing, print an install instruction and exit with code 1.
pub fn ensure_tool(name: &str, install_cmd: &str) {
    let found = Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    // Fallback for Windows
    let found = if !found {
        Command::new("where")
            .arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        found
    };

    if !found {
        eprintln!("{}", missing_tool_message(name, install_cmd));
        std::process::exit(1);
    }
}

/// Check if a tool binary is available on PATH (non-fatal).
pub fn has_tool(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or_else(|_| {
            // Fallback for Windows
            Command::new("where")
                .arg(name)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
}
