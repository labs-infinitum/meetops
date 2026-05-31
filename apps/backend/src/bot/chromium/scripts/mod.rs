//! JavaScript injected into the Meet tab via CDP `Runtime.evaluate`.
//!
//! Each `.js` file is the full expression passed to `Page::evaluate_function` (e.g. `async function() { … }`).
//! Edit scripts here for syntax highlighting and review; Rust embeds them at compile time via `include_str!`.
//!
//! Validate syntax: `node -e "new Function('return ('+require('fs').readFileSync('attempt_join.js','utf8')+')')"`
//!
//! Meet UI selectors must use roles, ARIA, stable `data-*` attributes, and user-visible
//! strings—not `jsname` / obfuscated class names (see `scripts/README.md`).

const ATTEMPT_JOIN: &str = include_str!("attempt_join.js");
const INSTALL_RECORDING_CONTROL: &str = include_str!("install_recording_control.js");
const RECORDING_STATUS: &str = include_str!("recording_status.js");
const MEET_PARTICIPANT_COUNT: &str = include_str!("meet_participant_count.js");
const RECORDING_RUNTIME_STATUS: &str = include_str!("recording_runtime_status.js");
const STOP_TAB_RECORDING: &str = include_str!("stop_tab_recording.js");
const PRIME_PARTICIPANT_ROSTER: &str = include_str!("prime_participant_roster.js");
const DISMISS_MEET_OVERLAYS: &str = include_str!("dismiss_meet_overlays.js");

/// Placeholder replaced with a JSON-string literal for the bot display name.
const DISPLAY_NAME_PLACEHOLDER: &str = "__DISPLAY_NAME_JSON__";

pub fn attempt_join(display_name: &str) -> anyhow::Result<String> {
    let quoted = serde_json::to_string(display_name)?;
    let script = ATTEMPT_JOIN.replace(DISPLAY_NAME_PLACEHOLDER, &quoted);
    if script.contains(DISPLAY_NAME_PLACEHOLDER) {
        anyhow::bail!("attempt_join.js: display name placeholder was not fully substituted");
    }
    Ok(script)
}

pub fn install_recording_control() -> &'static str {
    INSTALL_RECORDING_CONTROL
}

pub fn recording_status() -> &'static str {
    RECORDING_STATUS
}

pub fn meet_participant_count() -> &'static str {
    MEET_PARTICIPANT_COUNT
}

pub fn recording_runtime_status() -> &'static str {
    RECORDING_RUNTIME_STATUS
}

pub fn stop_tab_recording() -> &'static str {
    STOP_TAB_RECORDING
}

pub fn prime_participant_roster() -> &'static str {
    PRIME_PARTICIPANT_ROSTER
}

pub fn dismiss_meet_overlays() -> &'static str {
    DISMISS_MEET_OVERLAYS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripts must parse as function expressions (same shape CDP evaluates).
    fn assert_valid_function_expression(source: &str, name: &str) {
        if let Err(message) = js_syntax_check(source) {
            panic!("{name} is not valid JavaScript: {message}");
        }
    }

    fn js_syntax_check(source: &str) -> Result<(), String> {
        if !source.trim_end().ends_with('}') {
            return Err("script does not end with '}'".into());
        }
        let wrapped = format!("({source})");
        let path = std::env::temp_dir().join(format!(
            "gmr-script-check-{}.js",
            std::process::id()
        ));
        std::fs::write(&path, &wrapped).map_err(|e| e.to_string())?;
        let ok = std::process::Command::new("node")
            .arg("--check")
            .arg(&path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let _ = std::fs::remove_file(&path);
        if ok {
            return Ok(());
        }
        Err("node --check failed (wrap scripts as parenthesized expressions)".into())
    }

    #[test]
    fn all_scripts_end_with_closing_brace() {
        assert_valid_function_expression(ATTEMPT_JOIN, "attempt_join.js");
        assert_valid_function_expression(INSTALL_RECORDING_CONTROL, "install_recording_control.js");
        assert_valid_function_expression(RECORDING_STATUS, "recording_status.js");
        assert_valid_function_expression(MEET_PARTICIPANT_COUNT, "meet_participant_count.js");
        assert_valid_function_expression(RECORDING_RUNTIME_STATUS, "recording_runtime_status.js");
        assert_valid_function_expression(STOP_TAB_RECORDING, "stop_tab_recording.js");
        assert_valid_function_expression(PRIME_PARTICIPANT_ROSTER, "prime_participant_roster.js");
        assert_valid_function_expression(DISMISS_MEET_OVERLAYS, "dismiss_meet_overlays.js");
    }

    #[test]
    fn attempt_join_substitutes_display_name() {
        let script = attempt_join("Test Bot").expect("substitute");
        assert!(!script.contains(DISPLAY_NAME_PLACEHOLDER));
        assert!(script.contains("\"Test Bot\""));
    }
}
