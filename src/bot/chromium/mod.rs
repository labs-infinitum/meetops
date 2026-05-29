use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chromiumoxide::cdp::browser_protocol::browser::{
    PermissionDescriptor, PermissionSetting, SetPermissionParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{AddBindingParams, EventBindingCalled};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::Instant;

const PROFILE_MARKER: &str = "google-meeting-recorder-bot-profile";
const RECORDING_CHUNK_BINDING: &str = "__gmrRecordChunk";

/// Linux: store cookies on disk without GNOME Keyring (survives Xvfb / desktop logout).
#[cfg(target_os = "linux")]
const LINUX_CHROME_PASSWORD_STORE_ARG: &str = "--password-store=basic";

#[cfg(target_os = "linux")]
mod tab_capture_allow;

#[derive(Debug, Clone)]
pub struct JoinOptions {
    pub meet_url: String,
    pub display_name: String,
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub admission_timeout: Duration,
    /// When set on Linux (e.g. `":1"`), forwarded to Chrome and every `xdotool` subprocess as `DISPLAY`.
    pub x11_display: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfileOptions {
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
    /// Same as [`JoinOptions.x11_display`]: virtual X/Xvfb/Xvnc display (`":1"`).
    pub x11_display: Option<String>,
}

pub struct MeetingSession {
    browser: Browser,
    handler_task: JoinHandle<()>,
    pub page: Page,
    /// Copy of configured display for [`start_tab_recording`] / xdotool.
    pub x11_display: Option<String>,
}

impl MeetingSession {
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// OS pid of the browser process chromiumoxide spawned (Linux: scope `xdotool` to this instance).
    #[cfg(target_os = "linux")]
    pub fn launched_chrome_pid(&mut self) -> Option<u32> {
        self.browser
            .get_mut_child()
            .and_then(|child| child.as_mut_inner().id())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn launched_chrome_pid(&mut self) -> Option<u32> {
        None
    }

    pub async fn close(mut self) {
        match tokio::time::timeout(Duration::from_secs(5), self.browser.close()).await {
            Err(_) => tracing::warn!("timed out closing Chromium browser"),
            Ok(Err(error)) => tracing::warn!(error = %error, "failed to close Chromium browser"),
            Ok(Ok(_)) => {}
        }
        match tokio::time::timeout(Duration::from_secs(5), self.browser.wait()).await {
            Err(_) => tracing::warn!("timed out waiting for Chromium browser process to exit"),
            Ok(Err(error)) => tracing::warn!(
                error = %error,
                "failed awaiting Chromium browser process exit"
            ),
            Ok(Ok(status)) => {
                tracing::debug!(exit_status = ?status, "Chromium browser process exited");
            }
        }

        self.handler_task.abort();
        let _ = tokio::time::timeout(Duration::from_secs(1), self.handler_task).await;
    }
}

pub struct BrowserRecording {
    writer_task: JoinHandle<anyhow::Result<()>>,
}

impl BrowserRecording {
    pub async fn finish_or_abort(mut self, timeout_duration: Duration) -> anyhow::Result<()> {
        match tokio::time::timeout(timeout_duration, &mut self.writer_task).await {
            Ok(result) => result?,
            Err(_) => {
                self.writer_task.abort();
                let _ = self.writer_task.await;
                anyhow::bail!("recording chunk writer timed out")
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct JoinAttempt {
    state: JoinState,
    message: String,
    title: String,
    controls: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum JoinState {
    NeedsAction,
    AskedToJoin,
    Joined,
    Denied,
}

/// Applies [`Browser.setPermission`](https://chromedevtools.github.io/devtools-protocol/tot/Browser/#method-setPermission)
/// for Meet so automated runs can bias permission state toward grant.
///
/// The `permission.name` field must match [Blink `PermissionName`](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/modules/permissions/permission_descriptor.idl)
/// (`display-capture`), not chrome's separate [`PermissionType`](https://chromedevtools.github.io/devtools-protocol/tot/Browser/#type-PermissionType) strings (`displayCapture`).
///
/// Tab/screen capture audio is gated by [`getDisplayMedia`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia) together with capture; there is no separate Permissions-API `"audioCapture"` descriptor.
///
/// Enterprise policy / CLI flags (`--auto-accept-this-tab-capture`) may still be required for a silent picker; Wayland portals can block automation regardless.
pub async fn grant_meet_capture_permissions(browser: &Browser) -> anyhow::Result<()> {
    const MEET_ORIGIN: &str = "https://meet.google.com";
    let mut params = SetPermissionParams::new(
        PermissionDescriptor::new("display-capture".to_owned()),
        PermissionSetting::Granted,
    );
    params.origin = Some(MEET_ORIGIN.to_owned());
    browser.execute(params).await.map_err(|error| {
        anyhow::anyhow!(
            "Browser.setPermission(display-capture) [screen/tab capture] for Meet: {error}"
        )
    })?;
    tracing::info!(
        permission = "display-capture",
        origin = MEET_ORIGIN,
        "applied CDP Browser.setPermission"
    );
    Ok(())
}

pub async fn join_meeting(options: JoinOptions) -> anyhow::Result<()> {
    let session = open_joined_meeting(options).await?;
    tracing::info!("bot is in the meeting; press Ctrl+C to leave and close Chromium");
    tokio::signal::ctrl_c().await?;
    session.close().await;

    Ok(())
}

pub async fn open_joined_meeting(options: JoinOptions) -> anyhow::Result<MeetingSession> {
    let user_data_dir = profile_dir(&options.user_data_dir)?;
    tracing::info!(
        profile_dir = %user_data_dir.display(),
        marker = %user_data_dir.join(PROFILE_MARKER).display(),
        "using bot Chrome profile"
    );

    let (mut browser, mut handler) = Browser::launch(browser_config(
        &options.chromium_path,
        &user_data_dir,
        options.x11_display.as_deref(),
    )?)
    .await?;
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if let Err(error) = event {
                tracing::debug!(%error, "Chromium event handler error");
            }
        }
    });

    let result = run_join_flow(&browser, &options).await;

    match result {
        Ok(page) => Ok(MeetingSession {
            browser,
            handler_task,
            page,
            x11_display: options.x11_display.clone(),
        }),
        Err(error) => {
            match tokio::time::timeout(Duration::from_secs(5), browser.close()).await {
                Err(_) => tracing::warn!("timed out closing Chromium after join failure"),
                Ok(Err(close_err)) => tracing::warn!(
                    error = %close_err,
                    "failed closing Chromium after join failure"
                ),
                Ok(Ok(_)) => {}
            }
            match tokio::time::timeout(Duration::from_secs(5), browser.wait()).await {
                Err(_) => tracing::warn!("timed out waiting for Chromium after join failure"),
                Ok(Err(wait_err)) => tracing::warn!(
                    error = %wait_err,
                    "failed waiting for Chromium exit after join failure"
                ),
                Ok(Ok(_)) => {}
            }
            handler_task.abort();
            Err(error)
        }
    }
}

pub async fn open_profile(options: ProfileOptions) -> anyhow::Result<()> {
    let user_data_dir = profile_dir(&options.user_data_dir)?;
    warn_if_profile_lock_exists(&user_data_dir);
    let chrome_path = chrome_executable(&options.chromium_path)
        .ok_or_else(|| anyhow::anyhow!("Chrome/Chromium executable was not found"))?;

    let mut child_cmd = Command::new(&chrome_path);
    child_cmd
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--profile-directory=Default")
        .arg("--new-window")
        .arg("--no-first-run");
    #[cfg(target_os = "linux")]
    {
        child_cmd.arg(LINUX_CHROME_PASSWORD_STORE_ARG);
        tracing::info!(
            flag = LINUX_CHROME_PASSWORD_STORE_ARG,
            "Chrome profile uses on-disk cookie encryption (not session keyring)"
        );
    }
    child_cmd.arg("https://accounts.google.com/");
    if let Some(disp) = resolved_chrome_display(options.x11_display.as_deref())? {
        child_cmd.env("DISPLAY", disp);
    }
    let mut child = child_cmd.spawn()?;

    tracing::info!(
        chrome = %chrome_path.display(),
        profile_dir = %user_data_dir.display(),
        "opened normal Chrome profile; sign into Google, then close this Chrome window to save the profile"
    );

    let status = child.wait().await?;
    tracing::info!(?status, "Chrome profile setup process exited");

    Ok(())
}

pub fn profile_status(configured_dir: &PathBuf) -> anyhow::Result<()> {
    let user_data_dir = profile_dir(configured_dir)?;
    let cookies_path = user_data_dir.join("Default").join("Cookies");
    let preferences_path = user_data_dir.join("Default").join("Preferences");
    let local_state_path = user_data_dir.join("Local State");

    println!("profile_dir={}", user_data_dir.display());
    println!("marker={}", user_data_dir.join(PROFILE_MARKER).exists());
    println!("cookies_db={}", cookies_path.exists());
    println!("preferences={}", preferences_path.exists());
    println!("local_state={}", local_state_path.exists());

    if cookies_path.exists() {
        let metadata = std::fs::metadata(&cookies_path)?;
        println!("cookies_db_bytes={}", metadata.len());
    }

    Ok(())
}

fn browser_config(
    chromium_path: &PathBuf,
    user_data_dir: &PathBuf,
    x11_display: Option<&str>,
) -> anyhow::Result<BrowserConfig> {
    let chrome_path = chrome_executable(chromium_path);
    ensure_profile_dir(user_data_dir)?;

    let mut builder = BrowserConfig::builder()
        .disable_default_args()
        .with_head()
        .user_data_dir(user_data_dir)
        .viewport(None)
        .window_size(1920, 1080)
        .arg("--profile-directory=Default")
        .arg("--new-window")
        .arg("--window-position=0,0")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-session-crashed-bubble")
        .arg("--disable-features=DefaultBrowserPrompt")
        .arg("--disable-infobars")
        .arg("--disable-notifications")
        .arg("--use-fake-ui-for-media-stream")
        .arg("--auto-accept-this-tab-capture")
        .arg("--auto-select-desktop-capture-source=Google Meet")
        // Tab titles look like "xxx-yyy-zzz · Google Meet"; Brave may ignore these switches.
        .arg("--auto-select-tab-capture-source-by-title=Google Meet")
        .arg("--autoplay-policy=no-user-gesture-required")
        .arg("--mute-audio=false");
    #[cfg(target_os = "linux")]
    {
        builder = builder.arg(LINUX_CHROME_PASSWORD_STORE_ARG);
        tracing::info!(
            flag = LINUX_CHROME_PASSWORD_STORE_ARG,
            profile_dir = %user_data_dir.display(),
            "Chrome bot launches use on-disk cookie encryption (not session keyring)"
        );
    }

    // `chromiumoxide` forwards `process_envs` with `Command::envs`. Setting only `DISPLAY` would
    // supply an explicit env block without `XAUTHORITY` / `HOME` / etc., so Chrome still reports
    // "Missing X server or $DISPLAY". Seed from the current process, then force `DISPLAY`.
    let display = resolved_chrome_display(x11_display)?;
    for (k, v) in std::env::vars() {
        builder = builder.env(k, v);
    }
    if let Some(d) = display {
        builder = builder.env("DISPLAY", d);
    }

    if let Some(chrome_path) = chrome_path {
        builder = builder.chrome_executable(chrome_path);
    } else {
        tracing::warn!(
            configured_path = %chromium_path.display(),
            "configured Chromium path and common Chrome locations do not exist; falling back to chromiumoxide detection"
        );
    }

    builder.build().map_err(anyhow::Error::msg)
}

/// Value for the Chrome child `DISPLAY` env var.
///
/// On Linux, uses explicit `x11_display` or `$DISPLAY` (set by [`crate::bot::x11_display::prepare`]).
fn resolved_chrome_display(x11_display: Option<&str>) -> anyhow::Result<Option<String>> {
    use crate::bot::x11_display::normalize_x11_display;

    #[cfg(target_os = "linux")]
    {
        if let Some(d) = x11_display {
            let disp = normalize_x11_display(d);
            tracing::info!(%disp, "Chrome DISPLAY");
            return Ok(Some(disp));
        }
        Err(anyhow::anyhow!(
            "Linux: no X11 DISPLAY after prepare; this is an internal error"
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        if let Some(d) = x11_display {
            let disp = normalize_x11_display(d);
            tracing::info!(%disp, "Chrome DISPLAY");
            Ok(Some(disp))
        } else {
            Ok(None)
        }
    }
}

fn ensure_profile_dir(user_data_dir: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(user_data_dir)?;
    std::fs::write(user_data_dir.join(PROFILE_MARKER), b"bot profile\n")?;
    Ok(())
}

fn profile_dir(configured_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    ensure_profile_dir(configured_dir)?;
    let absolute = if configured_dir.is_absolute() {
        configured_dir.clone()
    } else {
        std::env::current_dir()?.join(configured_dir)
    };
    Ok(absolute.canonicalize().unwrap_or(absolute))
}

fn warn_if_profile_lock_exists(user_data_dir: &PathBuf) {
    let singleton_lock = user_data_dir.join("SingletonLock");
    if singleton_lock.exists() {
        tracing::warn!(
            lock = %singleton_lock.display(),
            "bot Chrome profile appears to be in use; close existing bot Chrome windows before signing in"
        );
    }
}

fn chrome_executable(configured_path: &PathBuf) -> Option<PathBuf> {
    [
        configured_path.as_path(),
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".as_ref(),
        "/Applications/Chromium.app/Contents/MacOS/Chromium".as_ref(),
        "/usr/bin/google-chrome".as_ref(),
        "/usr/bin/chromium".as_ref(),
        "/usr/bin/chromium-browser".as_ref(),
    ]
    .into_iter()
    .find(|path| path.exists())
    .map(PathBuf::from)
}

async fn run_join_flow(browser: &Browser, options: &JoinOptions) -> anyhow::Result<Page> {
    tracing::info!(url = %options.meet_url, "opening Google Meet");
    let page = browser.new_page(options.meet_url.as_str()).await?;
    close_other_pages(browser, &page).await?;
    activate_page(&page).await?;
    let deadline = Instant::now() + options.admission_timeout;
    let mut last_state: Option<JoinState> = None;

    loop {
        let attempt = attempt_join(&page, &options.display_name).await?;
        if last_state != Some(attempt.state) {
            tracing::info!(
                state = ?attempt.state,
                title = %attempt.title,
                message = %attempt.message,
                controls = ?attempt.controls,
                "Google Meet join status"
            );
            last_state = Some(attempt.state);
        }

        match attempt.state {
            JoinState::Joined => {
                close_other_pages(browser, &page).await?;
                activate_page(&page).await?;
                return Ok(page);
            }
            JoinState::AskedToJoin => {
                if Instant::now() >= deadline {
                    anyhow::bail!("timed out waiting for host admission");
                }
            }
            JoinState::Denied => {
                anyhow::bail!(
                    "Meet blocked the join: {}\n\nTry `cargo run -- profile`, sign into the bot Google account, invite that account to the meeting, then rerun `cargo run -- join <meet_url>`.",
                    attempt.message
                );
            }
            JoinState::NeedsAction => {}
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn close_other_pages(browser: &Browser, keep_page: &Page) -> anyhow::Result<()> {
    let keep_target_id = keep_page.target_id().clone();
    for page in browser.pages().await? {
        if page.target_id() == &keep_target_id {
            continue;
        }

        let url = page
            .url()
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "<unknown>".to_string());
        tracing::info!(%url, "closing extra Chrome tab before capture");
        if let Err(error) = page.close().await {
            tracing::warn!(%error, "failed to close extra Chrome tab");
        }
    }

    Ok(())
}

async fn activate_page(page: &Page) -> anyhow::Result<()> {
    page.bring_to_front().await?;
    page.activate().await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(())
}

async fn attempt_join(page: &Page, display_name: &str) -> anyhow::Result<JoinAttempt> {
    let script = format!(
        r#"async function() {{
  const displayName = {};
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const visible = (el) => {{
    if (!el) return false;
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    return style.visibility !== 'hidden' && style.display !== 'none' && rect.width > 0 && rect.height > 0;
  }};
  const norm = (text) => (text || '').replace(/\s+/g, ' ').trim().toLowerCase();
  const bodyText = norm(document.body ? document.body.innerText : '');
  const controls = Array.from(document.querySelectorAll('button, [role="button"], a')).filter(visible);
  const inputs = Array.from(document.querySelectorAll('input')).filter(visible);
  const controlText = (control) => norm([
    control.innerText,
    control.textContent,
    control.getAttribute('aria-label'),
    control.getAttribute('data-tooltip'),
    control.getAttribute('title')
  ].filter(Boolean).join(' '));
  const visibleControls = controls.map(controlText).filter(Boolean).slice(0, 20);
  const clickButton = (patterns) => {{
    const control = controls.find((candidate) => patterns.some((pattern) => controlText(candidate).includes(pattern)));
    if (!control) return false;
    control.click();
    return true;
  }};

  if (bodyText.includes('you cannot join this video call') ||
      bodyText.includes('you were denied') ||
      bodyText.includes('no longer in the meeting') ||
      bodyText.includes('meeting is full')) {{
    return {{ state: 'denied', message: document.body.innerText || 'Meet reports that the bot cannot join', title: document.title || '', controls: visibleControls }};
  }}

  if (clickButton(['continue without microphone and camera', 'continue without mic and camera'])) {{
    await sleep(500);
    return {{ state: 'needs_action', message: 'Clicked pre-join no-mic/no-camera prompt', title: document.title || '', controls: visibleControls }};
  }}

  const nameInput = inputs.find((input) => {{
    const label = norm([
      input.getAttribute('aria-label'),
      input.getAttribute('placeholder'),
      input.getAttribute('name')
    ].filter(Boolean).join(' '));
    return input.type !== 'hidden' && (label.includes('name') || label.includes('your name'));
  }});
  if (nameInput && nameInput.value !== displayName) {{
    nameInput.focus();
    nameInput.value = displayName;
    nameInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
    nameInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
    await sleep(250);
  }}

  clickButton(['turn off microphone', 'mute microphone']);
  clickButton(['turn off camera', 'turn off video']);

  const joinedSignals = [
    'leave call',
    'show everyone',
    'meeting details',
    'people are in this call',
    'you’re the only one here',
    "you're the only one here"
  ];
  if (joinedSignals.some((signal) => bodyText.includes(signal)) ||
      controls.some((control) => controlText(control).includes('leave call'))) {{
    return {{ state: 'joined', message: 'Joined meeting UI detected', title: document.title || '', controls: visibleControls }};
  }}

  if (bodyText.includes('asking to be let in') ||
      bodyText.includes('waiting to be let in') ||
      bodyText.includes('you’ll join when someone lets you in') ||
      bodyText.includes("you'll join when someone lets you in")) {{
    return {{ state: 'asked_to_join', message: 'Waiting for host admission', title: document.title || '', controls: visibleControls }};
  }}

  if (clickButton(['ask to join', 'join now'])) {{
    await sleep(500);
    return {{ state: 'asked_to_join', message: 'Clicked join button', title: document.title || '', controls: visibleControls }};
  }}

  return {{ state: 'needs_action', message: 'Waiting for Meet join controls', title: document.title || '', controls: visibleControls }};
}}"#,
        serde_json::to_string(display_name)?
    );

    Ok(page.evaluate_function(script).await?.into_value()?)
}

pub async fn start_tab_recording(
    page: &Page,
    raw_webm_path: impl AsRef<Path>,
    linux_xdotool_display: Option<String>,
    allow_button_template_png: Option<PathBuf>,
    tab_capture_screenshot_dir: PathBuf,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<BrowserRecording> {
    activate_page(page).await?;
    let recording = install_recording_binding(page, raw_webm_path.as_ref()).await?;
    install_recording_button(page).await?;
    let (click_tx, click_rx) = tokio::sync::oneshot::channel();
    let prompt_acceptor = spawn_tab_capture_prompt_acceptor_after_recorder_click(
        click_rx,
        linux_xdotool_display,
        allow_button_template_png,
        tab_capture_screenshot_dir,
        launched_chrome_pid,
    );

    match page.find_element("#gmr-start-recording").await {
        Ok(button) => {
            if let Err(error) = button.click().await {
                tracing::warn!(%error, "could not click recorder start trigger");
            } else {
                let _ = click_tx.send(());
            }
        }
        Err(error) => {
            tracing::warn!(%error, "could not auto-click recorder start button");
        }
    }

    // Poll quickly so we notice "recording" soon after the Allow click (500ms was very noticeable).
    for _ in 0..400 {
        let status = recording_status(page).await?;
        match status.get("status").and_then(|status| status.as_str()) {
            Some("recording") => {
                prompt_acceptor.abort();
                tracing::info!(%status, "browser MediaRecorder started");
                return Ok(recording);
            }
            Some("failed") => {
                prompt_acceptor.abort();
                anyhow::bail!("getDisplayMedia failed: {status}");
            }
            _ => tokio::time::sleep(std::time::Duration::from_millis(150)).await,
        }
    }

    prompt_acceptor.abort();
    anyhow::bail!(
        "timed out waiting for browser recording to start; Chrome tab-capture approval may still be open"
    )
}

#[cfg(target_os = "linux")]
fn spawn_tab_capture_prompt_acceptor_after_recorder_click(
    wait_for_click: tokio::sync::oneshot::Receiver<()>,
    linux_xdotool_display: Option<String>,
    allow_button_template_png: Option<PathBuf>,
    tab_capture_screenshot_dir: PathBuf,
    launched_chrome_pid: Option<u32>,
) -> JoinHandle<()> {
    tab_capture_allow::spawn(
        wait_for_click,
        linux_xdotool_display,
        allow_button_template_png,
        tab_capture_screenshot_dir,
        launched_chrome_pid,
    )
}

#[cfg(not(target_os = "linux"))]
fn spawn_tab_capture_prompt_acceptor_after_recorder_click(
    wait_for_click: tokio::sync::oneshot::Receiver<()>,
    linux_xdotool_display: Option<String>,
    #[allow(unused_variables)] allow_button_template_png: Option<PathBuf>,
    #[allow(unused_variables)] tab_capture_screenshot_dir: PathBuf,
    #[allow(unused_variables)] launched_chrome_pid: Option<u32>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let xdot_disp = linux_xdotool_display.as_deref();
        if wait_for_click.await.is_err() {
            tracing::debug!(
                "tab capture auto-accept not started (recorder trigger click skipped or canceled)"
            );
            return;
        }

        tokio::time::sleep(Duration::from_millis(75)).await;

        tracing::info!(
            x11_display_explicit = ?xdot_disp,
            process_display_env = ?std::env::var("DISPLAY").ok(),
            "auto-accepting tab-capture sheet in the background (macOS/other; template image unused)"
        );

        for attempt in 1..=48 {
            if let Err(error) = accept_tab_capture_prompt(xdot_disp).await {
                tracing::warn!(attempt, %error, "could not auto-accept Chrome tab capture prompt");
            } else {
                tracing::trace!(
                    attempt,
                    "tab capture auto-accept infra pass finished (not proof the Allow sheet was dismissed)"
                );
            }
            tokio::time::sleep(Duration::from_millis(85)).await;
        }
    })
}

#[cfg(target_os = "macos")]
async fn accept_tab_capture_prompt(_linux_xdotool_display: Option<&str>) -> anyhow::Result<()> {
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        Command::new("osascript")
            .arg("-e")
            .arg(
                r#"tell application "Google Chrome" to activate
delay 0.15
tell application "System Events"
  tell process "Google Chrome"
    set frontmost to true
    if (count of windows) is 0 then error "Google Chrome has no windows"
    try
      click button "Allow" of window 1
      return "clicked-button"
    end try
    try
      click button "Allow" of sheet 1 of window 1
      return "clicked-sheet-button"
    end try
    set windowPosition to position of window 1
    set windowSize to size of window 1
    set clickX to ((item 1 of windowPosition) + ((item 1 of windowSize) * 0.67)) as integer
    set clickY to ((item 2 of windowPosition) + ((item 2 of windowSize) * 0.57)) as integer
    click at {clickX, clickY}
    delay 0.1
    key code 36
    return "clicked-coordinate-and-pressed-return"
  end tell
end tell"#,
            )
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("osascript timed out; check macOS Accessibility permission"))??;

    if !output.status.success() {
        anyhow::bail!(
            "osascript failed: {}; allow Terminal/Codex in System Settings > Privacy & Security > Accessibility",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn linux_xdotool_apply_display(cmd: &mut Command, display: Option<&str>) {
    use crate::bot::x11_display::normalize_x11_display;
    if let Some(d) = display {
        cmd.env("DISPLAY", normalize_x11_display(d));
    }
}

/// Focus a window for screenshots/clicks. Uses `windowfocus` when `windowactivate` fails (Xvfb).
#[cfg(target_os = "linux")]
pub(super) async fn linux_xdotool_activate_window(
    timeout: Duration,
    display: Option<&str>,
    wid: &str,
) -> anyhow::Result<()> {
    let activate = tokio::time::timeout(timeout, {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["windowactivate", wid]).status()
    })
    .await
    .map_err(|_| anyhow::anyhow!("xdotool windowactivate timed out"))??;

    if activate.success() {
        return Ok(());
    }

    let focus = tokio::time::timeout(timeout, {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["windowfocus", wid]).status()
    })
    .await
    .map_err(|_| anyhow::anyhow!("xdotool windowfocus timed out"))??;

    if focus.success() {
        tracing::debug!(window_id = %wid, "xdotool windowactivate failed; used windowfocus");
        return Ok(());
    }

    anyhow::bail!("xdotool windowactivate and windowfocus failed for window {wid}")
}

/// Click `(origin + width * x_frac, origin + height * y_frac)`. Avoid `--sync`; it stalls on Wayland.
#[cfg(target_os = "linux")]
async fn linux_xdotool_move_click(
    subprocess_timeout: Duration,
    x_frac: f64,
    y_frac: f64,
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
    xdotool_display: Option<&str>,
) -> bool {
    let ax = origin_x + ((width as f64) * x_frac).round() as i32;
    let ay = origin_y + ((height as f64) * y_frac).round() as i32;
    let mut move_cmd = Command::new("xdotool");
    linux_xdotool_apply_display(&mut move_cmd, xdotool_display);
    let move_ok = tokio::time::timeout(
        subprocess_timeout,
        move_cmd
            .args(["mousemove", &ax.to_string(), &ay.to_string()])
            .status(),
    )
    .await;

    if !matches!(move_ok, Ok(Ok(status)) if status.success()) {
        return false;
    }

    tokio::time::sleep(Duration::from_millis(18)).await;
    let mut click_cmd = Command::new("xdotool");
    linux_xdotool_apply_display(&mut click_cmd, xdotool_display);
    matches!(
        tokio::time::timeout(
            subprocess_timeout,
            click_cmd.args(["click", "1"]).status(),
        )
        .await,
        Ok(Ok(c)) if c.success(),
    )
}

#[cfg(target_os = "linux")]
pub(super) async fn linux_accept_tab_capture_prompt(
    display: Option<&str>,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<()> {
    const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(5);

    let which = tokio::time::timeout(
        Duration::from_millis(500),
        Command::new("which").arg("xdotool").output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("which xdotool timed out"))??;

    if !which.status.success() {
        anyhow::bail!(
            "xdotool not found; install it (e.g. apt install xdotool) to auto-accept the tab-capture dialog on Linux, or use Google Chrome instead of Brave (Chromium devtools flags are more reliable)"
        );
    }

    let wid = tokio::time::timeout(
        SUBPROCESS_TIMEOUT,
        linux_find_largest_chromium_window(display, launched_chrome_pid),
    )
    .await
    .map_err(|_| anyhow::anyhow!("xdotool search timed out"))??;

    let wid_str = wid.to_string();

    linux_xdotool_activate_window(SUBPROCESS_TIMEOUT, display, &wid_str).await?;

    tokio::time::sleep(Duration::from_millis(210)).await;

    let geometry = tokio::time::timeout(SUBPROCESS_TIMEOUT, {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["getwindowgeometry", "--shell", &wid_str]).output()
    })
    .await
    .map_err(|_| anyhow::anyhow!("xdotool getwindowgeometry timed out"))??;

    if !geometry.status.success() {
        anyhow::bail!(
            "xdotool getwindowgeometry failed: {}",
            String::from_utf8_lossy(&geometry.stderr).trim()
        );
    }

    let mut gx: Option<i32> = None;
    let mut gy: Option<i32> = None;
    let mut gwidth: Option<i32> = None;
    let mut gheight: Option<i32> = None;
    for line in String::from_utf8_lossy(&geometry.stdout).lines() {
        if let Some(rest) = line.strip_prefix("X=") {
            gx = rest.parse().ok();
        } else if let Some(rest) = line.strip_prefix("Y=") {
            gy = rest.parse().ok();
        } else if let Some(rest) = line.strip_prefix("WIDTH=") {
            gwidth = rest.parse().ok();
        } else if let Some(rest) = line.strip_prefix("HEIGHT=") {
            gheight = rest.parse().ok();
        }
    }

    if let (Some(x), Some(y), Some(w), Some(h)) = (gx, gy, gwidth, gheight) {
        // Chrome stacks the tab-audio toggle above the Cancel/Allow row. Clicks ~0.6y often hit that
        // switch (kbd `Tab Return` activates whatever is focused — typically toggling audio off first).
        for (xf, yf) in [(0.90, 0.88), (0.88, 0.91), (0.92, 0.87), (0.86, 0.93)] {
            if linux_xdotool_move_click(SUBPROCESS_TIMEOUT, xf, yf, x, y, w, h, display).await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(38)).await;
        }
        // Inner-ish box biased to lower-right inside the viewport (dialogs sit mid-window).
        let dx = ((w as f64) * 0.05).round() as i32;
        let dy = ((h as f64) * 0.52).round() as i32;
        let cw = ((w as f64) * 0.92).round() as i32;
        let ch = ((h as f64) * 0.42).round() as i32;
        for (xf, yf) in [(0.88, 0.90), (0.92, 0.88)] {
            if linux_xdotool_move_click(SUBPROCESS_TIMEOUT, xf, yf, x + dx, y + dy, cw, ch, display)
                .await
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(38)).await;
        }
    }

    // Do not send `Tab` before `Return`: first Tab often focuses "Also share tab audio" and Return flips it off.
    for spec in ["shift+Tab Return", "Right Return", "Right Right Return"] {
        for use_window_flag in [false, true] {
            let mut cmd = Command::new("xdotool");
            linux_xdotool_apply_display(&mut cmd, display);
            cmd.arg("key");
            if use_window_flag {
                cmd.arg("--window").arg(&wid_str);
            }
            for part in spec.split_whitespace() {
                cmd.arg(part);
            }

            match tokio::time::timeout(SUBPROCESS_TIMEOUT, cmd.status()).await {
                Ok(Ok(status)) if !status.success() => {
                    tracing::debug!(?spec, window = use_window_flag, %status, "xdotool key failed");
                }
                Ok(Err(io)) => {
                    tracing::warn!(?spec, window = use_window_flag, %io, "xdotool key spawn failed")
                }
                Err(_) => tracing::warn!(?spec, window = use_window_flag, "xdotool key timed out"),
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(28)).await;
        }
    }

    tracing::debug!(
        "finished xdotool tab-capture pass; if Meet still prompts, enable Google Chrome automation flags or click Allow manually"
    );

    Ok(())
}

#[cfg(target_os = "linux")]
async fn xdotool_window_width_height_shell(display: Option<&str>, wid: u32) -> Option<(i32, i32)> {
    let out = match tokio::time::timeout(Duration::from_secs(5), async {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["getwindowgeometry", "--shell", &wid.to_string()])
            .output()
            .await
    })
    .await
    {
        Ok(Ok(o)) => o,
        _ => return None,
    };

    if !out.status.success() {
        return None;
    }

    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;
    for gline in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(rest) = gline.strip_prefix("WIDTH=") {
            width = rest.parse().ok();
        } else if let Some(rest) = gline.strip_prefix("HEIGHT=") {
            height = rest.parse().ok();
        }
    }
    match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
        _ => None,
    }
}

/// Prefer the largest pixel area among these window ids (visible stack order from `xdotool search`).
#[cfg(target_os = "linux")]
async fn linux_pick_largest_window_by_area(
    display: Option<&str>,
    wid_lines_stdout: &str,
) -> Option<u32> {
    let mut best: Option<(u32, u64)> = None;
    for line in wid_lines_stdout.lines() {
        let Ok(wid) = line.trim().parse::<u32>() else {
            continue;
        };
        let Some((w, h)) = xdotool_window_width_height_shell(display, wid).await else {
            continue;
        };
        let area = w as u64 * h as u64;
        if best.map_or(true, |(_, a)| area > a) {
            best = Some((wid, area));
        }
    }
    best.map(|(w, _)| w)
}

#[cfg(target_os = "linux")]
pub(super) async fn linux_find_largest_chromium_window(
    display: Option<&str>,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<u32> {
    if let Some(pid) = launched_chrome_pid {
        let output = {
            let mut c = Command::new("xdotool");
            linux_xdotool_apply_display(&mut c, display);
            c.args(["search", "--onlyvisible", "--pid", &pid.to_string()])
                .output()
        }
        .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(wid) = linux_pick_largest_window_by_area(display, stdout.as_ref()).await {
                tracing::debug!(
                    window_id = wid,
                    chrome_pid = pid,
                    "xdotool scoped to launched Chrome PID"
                );
                return Ok(wid);
            }
        }

        tracing::warn!(
            pid,
            "xdotool found no usable visible window for launched Chrome PID — falling back to WM_CLASS search (another Chrome window may win)"
        );
    }

    const CLASSES: &[&str] = &[
        "brave-browser",
        "Brave-browser",
        "brave",
        "Brave",
        "google-chrome",
        "Google-chrome",
        "chromium",
        "Chromium",
        "Chromium-browser",
    ];

    let mut best: Option<(u32, u64)> = None;
    for class in CLASSES {
        let output = {
            let mut c = Command::new("xdotool");
            linux_xdotool_apply_display(&mut c, display);
            c.args(["search", "--class", "--onlyvisible", class])
                .output()
        }
        .await?;

        if !output.status.success() {
            continue;
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(wid) = line.trim().parse::<u32>() else {
                continue;
            };
            let Some((width, height)) = xdotool_window_width_height_shell(display, wid).await
            else {
                continue;
            };
            let area = width as u64 * height as u64;
            if best.map_or(true, |(_, a)| area > a) {
                best = Some((wid, area));
            }
        }
    }

    best.map(|(w, _)| w).ok_or_else(|| {
        anyhow::anyhow!(
            "xdotool found no visible Brave/Chrome/Chromium window; is the automation browser running?"
        )
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn accept_tab_capture_prompt(_linux_xdotool_display: Option<&str>) -> anyhow::Result<()> {
    anyhow::bail!(
        "automatic tab-capture prompt acceptance is not implemented on this OS; use macOS or Linux with xdotool, or click Allow manually when the prompt appears"
    )
}

async fn install_recording_binding(
    page: &Page,
    raw_webm_path: &Path,
) -> anyhow::Result<BrowserRecording> {
    let mut listener = page.event_listener::<EventBindingCalled>().await?;
    page.execute(AddBindingParams::new(RECORDING_CHUNK_BINDING))
        .await?;

    let raw_webm_path = raw_webm_path.to_path_buf();
    let writer_task = tokio::spawn(async move {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_webm_path)
            .await?;
        let mut chunks = 0usize;
        let mut bytes = 0usize;

        while let Some(event) = listener.next().await {
            if event.name != RECORDING_CHUNK_BINDING {
                continue;
            }

            let payload = match serde_json::from_str::<serde_json::Value>(&event.payload) {
                Ok(payload) => payload,
                Err(error) => {
                    tracing::warn!(%error, "ignored malformed recording chunk payload");
                    continue;
                }
            };

            match payload.get("kind").and_then(|kind| kind.as_str()) {
                Some("chunk") => {
                    let Some(data) = payload.get("data").and_then(|data| data.as_str()) else {
                        tracing::warn!("ignored recording chunk without data");
                        continue;
                    };
                    let decoded = BASE64.decode(data.as_bytes())?;
                    let declared_size = payload
                        .get("size")
                        .and_then(|size| size.as_u64())
                        .unwrap_or(decoded.len() as u64);
                    if decoded.len() as u64 != declared_size {
                        tracing::warn!(
                            decoded_bytes = decoded.len(),
                            declared_size,
                            "recording chunk size mismatch"
                        );
                    }

                    let participant_count = payload.get("participantCount").and_then(|v| {
                        v.as_u64().or_else(|| {
                            v.as_i64()
                                .filter(|&i| i >= 0)
                                .map(|i| i as u64)
                        })
                    });
                    let participant_source = payload
                        .get("participantSource")
                        .and_then(|v| v.as_str());

                    file.write_all(&decoded).await?;
                    file.flush().await?;
                    chunks += 1;
                    bytes += decoded.len();
                    tracing::info!(
                        chunks,
                        bytes,
                        participants = participant_count,
                        participant_source,
                        "accepted recording chunk with Meet participant snapshot (best-effort)"
                    );
                }
                Some("done") => {
                    file.flush().await?;
                    tracing::info!(chunks, bytes, "recording chunk writer finished");
                    return Ok(());
                }
                other => {
                    tracing::warn!(?other, "ignored unknown recording binding event");
                }
            }
        }

        file.flush().await?;
        tracing::info!(chunks, bytes, "recording chunk writer ended");
        Ok(())
    });

    Ok(BrowserRecording { writer_task })
}

async fn install_recording_button(page: &Page) -> anyhow::Result<()> {
    let script = r#"async function() {
  function gmrMeetParticipantCount() {
    const out = { count: null, source: null, raw: null };
    try {
      const withLabels = document.querySelectorAll('[aria-label]');
      for (let i = 0; i < withLabels.length; i++) {
        const label = (withLabels[i].getAttribute('aria-label') || '').trim();
        if (!label) continue;
        let m = label.match(/(\d+)\s+(people|participants)/i);
        if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_people'; out.raw = label; return out; }
        m = label.match(/(?:people|participants)\s*[-–—:]\s*(\d+)/i);
        if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_dash'; out.raw = label; return out; }
        m = label.match(/(\d+)\s+joined/i);
        if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_joined'; out.raw = label; return out; }
      }
      const body = document.body && document.body.innerText ? document.body.innerText : '';
      if (body) {
        const m = body.match(/\b(\d+)\s+people\s+in\s+this\s+call\b/i);
        if (m) { out.count = parseInt(m[1], 10); out.source = 'body_text'; return out; }
      }
      const tiles = document.querySelectorAll('[data-participant-id], [data-requested-participant-id]');
      if (tiles.length) {
        const seen = {};
        for (let i = 0; i < tiles.length; i++) {
          const t = tiles[i];
          const id = t.getAttribute('data-participant-id') || t.getAttribute('data-requested-participant-id');
          if (id) seen[id] = true;
        }
        const n = Object.keys(seen).length;
        if (n > 0) { out.count = n; out.source = 'tiles'; return out; }
      }
    } catch (e) {
      out.source = 'error';
      out.raw = String(e && e.message ? e.message : e);
    }
    return out;
  }
  window.__gmrRecordingStatus = { status: 'waiting_for_click' };
  if (window.__gmrRecorder && window.__gmrRecorder.state === 'recording') {
    window.__gmrRecordingStatus = { status: 'recording', mimeType: window.__gmrRecorder.mimeType || '' };
    return window.__gmrRecordingStatus;
  }

  let button = document.querySelector('#gmr-start-recording');
  if (!button) {
    button = document.createElement('button');
    button.id = 'gmr-start-recording';
    button.textContent = '';
    button.setAttribute('aria-label', 'Start AI recording');
    button.style.cssText = [
      'position:fixed',
      'z-index:2147483647',
      'left:50%',
      'top:50%',
      'width:240px',
      'height:120px',
      'padding:0',
      'border:0',
      'margin:0',
      'opacity:0',
      'background:transparent',
      'transform:translate(-50%, -50%)',
      'overflow:hidden',
      'pointer-events:auto'
    ].join(';');
    document.body.appendChild(button);
  }

  button.onclick = async () => {
    button.disabled = true;
    button.style.pointerEvents = 'none';
    window.__gmrRecordingStatus = { status: 'starting' };

    let stream;
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({
        video: {
          displaySurface: 'browser',
          logicalSurface: true,
          cursor: 'always'
        },
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
          suppressLocalAudioPlayback: false
        },
        preferCurrentTab: true,
        selfBrowserSurface: 'include',
        systemAudio: 'include',
        surfaceSwitching: 'exclude'
      });
    } catch (error) {
      window.__gmrRecordingStatus = {
        status: 'failed',
        name: error && error.name ? error.name : 'Error',
        message: error && error.message ? error.message : String(error)
      };
      button.disabled = false;
      button.style.pointerEvents = 'auto';
      return;
    }

    button.remove();

    const mimeTypes = [
      'video/webm;codecs=vp8,opus',
      'video/webm;codecs=vp9,opus',
      'video/webm'
    ];
    const mimeType = mimeTypes.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '';
    const audioTracks = stream.getAudioTracks();
    const videoTracks = stream.getVideoTracks();
    const streamInfo = {
      audioTracks: audioTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      videoTracks: videoTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} }))
    };
    const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
    window.__gmrRecorder = recorder;
    window.__gmrRecorderStream = stream;
    window.__gmrStreamInfo = streamInfo;
    window.__gmrUploads = [];
    window.__gmrUploadErrors = [];
    window.__gmrChunkCount = 0;
    window.__gmrAcceptedChunks = 0;
    window.__gmrUploadedBytes = 0;
    window.__gmrPendingUpload = Promise.resolve();
    window.__gmrChunkBindingAvailable = typeof window.__gmrRecordChunk === 'function';
    if (audioTracks.length === 0) {
      window.__gmrUploadErrors.push('capture stream has no audio track; choose the Meet tab and keep "Also share tab audio" enabled');
    }

    const blobToBase64 = async (blob) => {
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let binary = '';
      const stride = 0x8000;
      for (let offset = 0; offset < bytes.length; offset += stride) {
        binary += String.fromCharCode.apply(null, bytes.subarray(offset, offset + stride));
      }
      return btoa(binary);
    };

    recorder.ondataavailable = (event) => {
      if (!event.data || event.data.size === 0) return;
      window.__gmrChunkCount += 1;
      const chunk = event.data;
      const upload = window.__gmrPendingUpload.then(async () => {
        if (typeof window.__gmrRecordChunk !== 'function') {
          throw new Error('recording DevTools binding is unavailable');
        }
        const data = await blobToBase64(chunk);
        const pc = gmrMeetParticipantCount();
        if (typeof pc.count === 'number' && !Number.isNaN(pc.count)) {
          window.__gmrLastParticipantCount = pc.count;
          window.__gmrLastParticipantSource = pc.source || null;
        }
        window.__gmrRecordChunk(JSON.stringify({
          kind: 'chunk',
          sequence: window.__gmrChunkCount,
          size: chunk.size,
          data,
          participantCount: pc.count,
          participantSource: pc.source
        }));
        window.__gmrAcceptedChunks += 1;
        window.__gmrUploadedBytes += chunk.size;
      }).catch((error) => {
        window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
      });
      window.__gmrPendingUpload = upload;
      window.__gmrUploads.push(upload);
    };

    recorder.start(1000);
    window.__gmrRecordingStatus = { status: 'recording', mimeType, streamInfo };
  };

  return window.__gmrRecordingStatus;
}"#;

    let result: serde_json::Value = page.evaluate_function(script).await?.into_value()?;
    tracing::info!(%result, "installed browser recording control");
    Ok(())
}

async fn recording_status(page: &Page) -> anyhow::Result<serde_json::Value> {
    Ok(page
        .evaluate_function(
            r#"function() {
  return window.__gmrRecordingStatus || { status: 'not_installed' };
}"#,
        )
        .await?
        .into_value()?)
}

pub async fn recording_runtime_status(page: &Page) -> anyhow::Result<serde_json::Value> {
    Ok(page
        .evaluate_function(
            r#"function() {
  const base = window.__gmrRecordingStatus || { status: 'not_installed' };
  return {
    ...base,
    chunks: window.__gmrChunkCount || 0,
    uploadedBytes: window.__gmrUploadedBytes || 0,
    acceptedChunks: window.__gmrAcceptedChunks || 0,
    uploadErrors: window.__gmrUploadErrors || [],
    bindingAvailable: !!window.__gmrChunkBindingAvailable,
    streamInfo: window.__gmrStreamInfo || null,
    lastParticipantCount: typeof window.__gmrLastParticipantCount === 'number' ? window.__gmrLastParticipantCount : null,
    lastParticipantSource: window.__gmrLastParticipantSource || null
  };
}"#,
        )
        .await?
        .into_value()?)
}

pub async fn stop_tab_recording(page: &Page) -> anyhow::Result<()> {
    let script = r#"async function() {
  const recorder = window.__gmrRecorder;
  if (!recorder) return { status: 'not_started', errors: [] };

  if (recorder.state !== 'inactive') {
    await new Promise((resolve) => {
      const done = () => resolve();
      recorder.addEventListener('stop', done, { once: true });
      recorder.requestData();
      recorder.stop();
    });
  }

  if (window.__gmrRecorderStream) {
    window.__gmrRecorderStream.getTracks().forEach((track) => track.stop());
  }
  await Promise.allSettled(window.__gmrUploads || []);
  if (typeof window.__gmrRecordChunk === 'function') {
    window.__gmrRecordChunk(JSON.stringify({ kind: 'done' }));
  }
  return { status: 'stopped', errors: window.__gmrUploadErrors || [] };
}"#;

    let result: serde_json::Value = page.evaluate_function(script).await?.into_value()?;
    if let Some(errors) = result.get("errors").and_then(|errors| errors.as_array()) {
        if !errors.is_empty() {
            anyhow::bail!("browser upload errors: {errors:?}");
        }
    }
    tracing::info!(%result, "browser MediaRecorder stopped");
    Ok(())
}
