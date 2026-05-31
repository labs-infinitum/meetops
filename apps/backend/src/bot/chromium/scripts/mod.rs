//! JavaScript injected into the Meet tab via CDP `Runtime.evaluate`.
//!
//! Each `.js` file is the full expression passed to `Page::evaluate_function` (e.g. `async function() { … }`).
//! Edit scripts here for syntax highlighting and review; Rust embeds them at compile time via `include_str!`.

const ATTEMPT_JOIN: &str = include_str!("attempt_join.js");
const INSTALL_RECORDING_CONTROL: &str = include_str!("install_recording_control.js");
const RECORDING_STATUS: &str = include_str!("recording_status.js");
const MEET_PARTICIPANT_COUNT: &str = include_str!("meet_participant_count.js");
const RECORDING_RUNTIME_STATUS: &str = include_str!("recording_runtime_status.js");
const STOP_TAB_RECORDING: &str = include_str!("stop_tab_recording.js");

/// Placeholder replaced with a JSON-string literal for the bot display name.
const DISPLAY_NAME_PLACEHOLDER: &str = "__DISPLAY_NAME_JSON__";

pub fn attempt_join(display_name: &str) -> anyhow::Result<String> {
    let quoted = serde_json::to_string(display_name)?;
    Ok(ATTEMPT_JOIN.replace(DISPLAY_NAME_PLACEHOLDER, &quoted))
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
