use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chromiumoxide::browser::BrowserConfigBuilder;
use chromiumoxide::cdp::browser_protocol::browser::{
    PermissionDescriptor, PermissionSetting, SetPermissionParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{AddBindingParams, EventBindingCalled};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::{
    bot::recording_status::{
        RecordingBindingPayload, RecordingPhase, RecordingRuntimeStatus, RecordingStatus,
    },
    db::meeting_timeline,
    meet_url::MeetUrl,
};

const PROFILE_MARKER: &str = "google-meeting-recorder-bot-profile";
const RECORDING_CHUNK_BINDING: &str = "__gmrRecordChunk";

/// Linux: store cookies on disk without GNOME Keyring (survives Xvfb / desktop logout).
#[cfg(target_os = "linux")]
const LINUX_CHROME_PASSWORD_STORE_ARG: &str = "--password-store=basic";

#[cfg(target_os = "linux")]
mod tab_capture_allow;

#[cfg(target_os = "linux")]
use crate::bot::xdotool_geom;

#[derive(Debug, Clone)]
pub struct JoinOptions {
    pub meet_url: MeetUrl,
    pub display_name: String,
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub admission_timeout: Duration,
    /// When set on Linux (e.g. `":1"`), forwarded to Chrome and every `xdotool` subprocess as `DISPLAY`.
    pub x11_display: Option<String>,
}

impl JoinOptions {
    pub fn from_config(
        meet_url: MeetUrl,
        config: &crate::config::Config,
        x11_display: Option<String>,
    ) -> Self {
        Self {
            meet_url,
            display_name: config.bot.display_name.clone(),
            chromium_path: config.bot.chromium_path.clone(),
            user_data_dir: config.bot.chrome_user_data_dir.clone(),
            admission_timeout: Duration::from_secs(config.bot.admission_timeout_secs),
            x11_display,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileOptions {
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
    /// Same as [`JoinOptions.x11_display`]: virtual X/Xvfb/Xvnc display (`":1"`).
    pub x11_display: Option<String>,
}

impl ProfileOptions {
    pub fn from_config(config: &crate::config::Config, x11_display: Option<String>) -> Self {
        Self {
            chromium_path: config.bot.chromium_path.clone(),
            user_data_dir: config.bot.chrome_user_data_dir.clone(),
            x11_display,
        }
    }
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
        graceful_browser_shutdown(&mut self.browser, "browser").await;
        self.handler_task.abort();
        let _ = tokio::time::timeout(Duration::from_secs(1), self.handler_task).await;
    }
}

async fn graceful_browser_shutdown(browser: &mut Browser, context: &str) {
    match tokio::time::timeout(Duration::from_secs(5), browser.close()).await {
        Err(_) => tracing::warn!(context, "timed out closing Chromium"),
        Ok(Err(error)) => tracing::warn!(error = %error, context, "failed closing Chromium"),
        Ok(Ok(_)) => {}
    }
    match tokio::time::timeout(Duration::from_secs(5), browser.wait()).await {
        Err(_) => tracing::warn!(context, "timed out waiting for Chromium process exit"),
        Ok(Err(error)) => tracing::warn!(error = %error, context, "failed awaiting Chromium exit"),
        Ok(Ok(status)) => {
            tracing::debug!(exit_status = ?status, context, "Chromium process exited");
        }
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
            graceful_browser_shutdown(&mut browser, "after join failure").await;
            handler_task.abort();
            Err(error)
        }
    }
}

pub async fn open_profile(options: ProfileOptions) -> anyhow::Result<()> {
    let user_data_dir = profile_dir(&options.user_data_dir)?;
    warn_if_profile_lock_exists(&user_data_dir);
    let chrome_path = chrome_executable(&options.chromium_path)?;

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
    #[cfg(target_os = "linux")]
    if running_in_container() {
        child_cmd
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-gpu");
        tracing::info!(
            "Chrome profile container flags enabled (--no-sandbox, --disable-dev-shm-usage)"
        );
    }
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
    let chrome_path = chrome_executable(chromium_path)?;
    ensure_profile_dir(user_data_dir)?;

    let mut builder = BrowserConfig::builder()
        .disable_default_args()
        .with_head()
        .user_data_dir(user_data_dir)
        .viewport(None)
        .window_size(1920, 1080);
    builder = apply_chromeoxide_flags(
        builder,
        &[
            "--profile-directory=Default",
            "--new-window",
            "--window-position=0,0",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-session-crashed-bubble",
            "--disable-restore-session-state",
            "--noerrdialogs",
            "--disable-features=DefaultBrowserPrompt,TranslateUI",
            "--disable-infobars",
            "--disable-notifications",
            "--use-fake-ui-for-media-stream",
            "--auto-accept-this-tab-capture",
            "--auto-select-desktop-capture-source=Google Meet",
            "--auto-select-tab-capture-source-by-title=Google Meet",
            "--autoplay-policy=no-user-gesture-required",
            "--mute-audio=false",
        ],
    );
    #[cfg(target_os = "linux")]
    {
        builder = apply_chromeoxide_flags(builder, &[LINUX_CHROME_PASSWORD_STORE_ARG]);
        if running_in_container() {
            builder = builder
                .no_sandbox()
                .arg("disable-dev-shm-usage")
                .arg("disable-gpu")
                .arg("no-zygote");
            tracing::info!("Chrome container flags enabled (no_sandbox, disable-dev-shm-usage)");
        }
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

    builder = builder.chrome_executable(chrome_path);

    builder.build().map_err(anyhow::Error::msg)
}

/// Chromiumoxide adds `--` itself; passing `"--no-sandbox"` would become `----no-sandbox`.
fn apply_chromeoxide_flags(
    mut builder: BrowserConfigBuilder,
    flags: &[&str],
) -> BrowserConfigBuilder {
    for flag in flags {
        let stripped = flag.strip_prefix("--").unwrap_or(flag);
        builder = if let Some((key, value)) = stripped.split_once('=') {
            builder.arg((key, value))
        } else {
            builder.arg(stripped)
        };
    }
    builder
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
    clear_stale_chrome_profile_locks(user_data_dir);
    clear_chrome_session_restore_artifacts(user_data_dir);
    std::fs::write(user_data_dir.join(PROFILE_MARKER), b"bot profile\n")?;
    Ok(())
}

/// Remove stale Chrome profile locks left behind by previous Docker containers.
///
/// Chrome's `SingletonLock` is stored inside the persisted user data dir. With `./data` bind-mounted,
/// an unclean container shutdown can leave a lock like `old-container-hostname-85`, causing the next
/// container to fail before DevTools is available. If the lock points at a live Chrome process on this
/// same host/container we keep it and let Chrome fail loudly.
fn clear_stale_chrome_profile_locks(user_data_dir: &Path) {
    let singleton_lock = user_data_dir.join("SingletonLock");
    if std::fs::symlink_metadata(&singleton_lock).is_err() {
        return;
    }

    if chrome_singleton_lock_appears_live(singleton_lock.as_path()) {
        tracing::warn!(
            lock = %singleton_lock.display(),
            "bot Chrome profile appears to be in use by a live Chrome process; not removing lock"
        );
        return;
    }

    for name in ["SingletonLock", "SingletonSocket", "SingletonCookie"] {
        let path = user_data_dir.join(name);
        if std::fs::symlink_metadata(&path).is_err() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(
                path = %path.display(),
                "removed stale Chrome profile singleton lock"
            ),
            Err(error) => tracing::warn!(
                path = %path.display(),
                %error,
                "could not remove stale Chrome profile singleton lock"
            ),
        }
    }
}

fn chrome_singleton_lock_appears_live(lock: &Path) -> bool {
    let Ok(target) = std::fs::read_link(lock) else {
        // A non-symlink lock in a container profile is more likely stale than intentionally live.
        return false;
    };
    let Some(target) = target.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some((host, pid_s)) = target.rsplit_once('-') else {
        return false;
    };
    let Ok(pid) = pid_s.parse::<u32>() else {
        return false;
    };

    let current_host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
        .unwrap_or_default();
    host == current_host && Path::new("/proc").join(pid.to_string()).exists()
}

/// Remove files Chrome uses to show "Restore pages?" after an unclean shutdown.
fn clear_chrome_session_restore_artifacts(user_data_dir: &Path) {
    let default_profile = user_data_dir.join("Default");
    for name in [
        "Last Session",
        "Last Tabs",
        "Current Session",
        "Current Tabs",
    ] {
        let path = default_profile.join(name);
        if !path.is_file() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!(
                path = %path.display(),
                "removed Chrome session restore artifact"
            ),
            Err(error) => tracing::debug!(
                path = %path.display(),
                %error,
                "could not remove Chrome session restore artifact"
            ),
        }
    }
}

fn profile_dir(configured_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    ensure_profile_dir(configured_dir)?;
    let absolute = if configured_dir.is_absolute() {
        configured_dir.clone()
    } else {
        std::env::current_dir()?.join(configured_dir)
    };
    match absolute.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(absolute),
        Err(error) => Err(error.into()),
    }
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

fn running_in_container() -> bool {
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

fn chrome_executable(configured_path: &PathBuf) -> anyhow::Result<PathBuf> {
    if configured_path.is_file() {
        Ok(configured_path.clone())
    } else {
        anyhow::bail!(
            "Chrome executable not found at {}; set [bot].chromium_path in config.toml",
            configured_path.display()
        )
    }
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

        let url = match page.url().await {
            Ok(Some(url)) => url,
            Ok(None) => "<no url>".to_owned(),
            Err(error) => {
                tracing::warn!(%error, "failed to read Chrome tab URL before close");
                "<unknown>".to_owned()
            }
        };
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
    audio_webm_path: impl AsRef<Path>,
    timeline_sink: Option<(SqlitePool, i64)>,
    linux_xdotool_display: Option<String>,
    tab_capture_screenshot_dir: PathBuf,
    tab_capture_png: PathBuf,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<BrowserRecording> {
    activate_page(page).await?;
    let recording = install_recording_binding(
        page,
        raw_webm_path.as_ref(),
        audio_webm_path.as_ref(),
        timeline_sink,
    )
    .await?;
    install_recording_button(page).await?;
    let (click_tx, click_rx) = tokio::sync::oneshot::channel();
    let prompt_acceptor = spawn_tab_capture_prompt_acceptor_after_recorder_click(
        click_rx,
        linux_xdotool_display,
        tab_capture_screenshot_dir,
        tab_capture_png,
        launched_chrome_pid,
    );

    match page.find_element("#gmr-start-recording").await {
        Ok(button) => {
            if let Err(error) = button.click().await {
                tracing::warn!(%error, "could not click recorder start trigger");
            } else if click_tx.send(()).is_err() {
                tracing::warn!("tab-capture prompt acceptor exited before recorder click signal");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "could not auto-click recorder start button");
        }
    }

    // Poll quickly so we notice "recording" soon after the Allow click (500ms was very noticeable).
    for _ in 0..400 {
        let status = recording_status(page).await?;
        match status.phase() {
            RecordingPhase::Recording => {
                prompt_acceptor.abort();
                tracing::info!(?status, "browser MediaRecorder started");
                return Ok(recording);
            }
            RecordingPhase::Failed => {
                prompt_acceptor.abort();
                match status {
                    RecordingStatus::Failed { name, message } => {
                        anyhow::bail!("getDisplayMedia failed: {name}: {message}");
                    }
                    other => anyhow::bail!("getDisplayMedia failed: {other:?}"),
                }
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
    tab_capture_screenshot_dir: PathBuf,
    tab_capture_png: PathBuf,
    launched_chrome_pid: Option<u32>,
) -> JoinHandle<()> {
    tab_capture_allow::spawn(
        wait_for_click,
        linux_xdotool_display,
        tab_capture_screenshot_dir,
        tab_capture_png,
        launched_chrome_pid,
    )
}

#[cfg(not(target_os = "linux"))]
fn spawn_tab_capture_prompt_acceptor_after_recorder_click(
    wait_for_click: tokio::sync::oneshot::Receiver<()>,
    linux_xdotool_display: Option<String>,
    #[allow(unused_variables)] tab_capture_screenshot_dir: PathBuf,
    #[allow(unused_variables)] tab_capture_png: PathBuf,
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
            "xdotool not found; install it (e.g. apt install xdotool) to auto-accept the tab-capture dialog on Linux"
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

    let geo = xdotool_geom::parse_geom_shell_stdout(&String::from_utf8_lossy(&geometry.stdout));

    if let (Some(x), Some(y), Some(w), Some(h)) = (geo.x, geo.y, geo.width, geo.height) {
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

    let geom = xdotool_geom::parse_geom_shell_stdout(&String::from_utf8_lossy(&out.stdout));
    match (geom.width, geom.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
        _ => None,
    }
}

/// Prefer the largest pixel area among these window ids (visible stack order from `xdotool search`).
#[cfg(target_os = "linux")]
async fn linux_pick_largest_window_by_area(
    display: Option<&str>,
    wid_lines_stdout: &str,
) -> Option<(u32, u64)> {
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
    best
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
            if let Some((wid, _)) =
                linux_pick_largest_window_by_area(display, stdout.as_ref()).await
            {
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

    const CLASSES: &[&str] = &["google-chrome", "Google-chrome"];

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

        if let Some(candidate) =
            linux_pick_largest_window_by_area(display, &String::from_utf8_lossy(&output.stdout))
                .await
        {
            if best.map_or(true, |(_, a)| candidate.1 > a) {
                best = Some(candidate);
            }
        }
    }

    best.map(|(w, _)| w).ok_or_else(|| {
        anyhow::anyhow!(
            "xdotool found no visible Google Chrome window; is the automation browser running?"
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
    audio_webm_path: &Path,
    timeline_sink: Option<(SqlitePool, i64)>,
) -> anyhow::Result<BrowserRecording> {
    let mut listener = page.event_listener::<EventBindingCalled>().await?;
    page.execute(AddBindingParams::new(RECORDING_CHUNK_BINDING))
        .await?;

    let raw_webm_path = raw_webm_path.to_path_buf();
    let audio_webm_path = audio_webm_path.to_path_buf();
    let writer_task = tokio::spawn(async move {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_webm_path)
            .await?;
        let mut audio_file: Option<tokio::fs::File> = None;
        let mut chunks = 0usize;
        let mut bytes = 0usize;
        let mut audio_chunks = 0usize;
        let mut audio_bytes = 0usize;

        while let Some(event) = listener.next().await {
            if event.name != RECORDING_CHUNK_BINDING {
                continue;
            }

            let payload = match serde_json::from_str::<RecordingBindingPayload>(&event.payload) {
                Ok(payload) => payload,
                Err(error) => {
                    tracing::warn!(%error, "ignored malformed recording chunk payload");
                    continue;
                }
            };

            match payload {
                RecordingBindingPayload::Chunk {
                    size,
                    data,
                    participant_count,
                    participant_source,
                    ..
                } => {
                    let decoded = BASE64.decode(data.as_bytes())?;
                    if decoded.len() as u64 != size {
                        tracing::warn!(
                            decoded_bytes = decoded.len(),
                            declared_size = size,
                            "recording chunk size mismatch"
                        );
                    }

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
                RecordingBindingPayload::AudioChunk { size, data } => {
                    let decoded = BASE64.decode(data.as_bytes())?;
                    if decoded.len() as u64 != size {
                        tracing::warn!(
                            decoded_bytes = decoded.len(),
                            declared_size = size,
                            "audio chunk size mismatch"
                        );
                    }

                    if audio_file.is_none() {
                        audio_file = Some(
                            tokio::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&audio_webm_path)
                                .await?,
                        );
                    }
                    let file = audio_file
                        .as_mut()
                        .expect("audio_file initialized before write");
                    file.write_all(&decoded).await?;
                    file.flush().await?;
                    audio_chunks += 1;
                    audio_bytes += decoded.len();
                    tracing::debug!(audio_chunks, audio_bytes, "accepted recording audio chunk");
                }
                RecordingBindingPayload::TimelineEvents { events } => {
                    let event_count = events.len();
                    if let Some((pool, run_id)) = timeline_sink.as_ref() {
                        let events = events.into_iter().map(Into::into).collect::<Vec<_>>();
                        if let Err(error) =
                            meeting_timeline::insert_many(pool, *run_id, &events).await
                        {
                            tracing::warn!(
                                %error,
                                run_id,
                                event_count,
                                "failed to persist meeting timeline events"
                            );
                        } else {
                            tracing::debug!(
                                run_id,
                                event_count,
                                "persisted meeting timeline events"
                            );
                        }
                    } else {
                        tracing::trace!(
                            event_count,
                            "ignored meeting timeline events for manual recording"
                        );
                    }
                }
                RecordingBindingPayload::Done => {
                    file.flush().await?;
                    if let Some(file) = audio_file.as_mut() {
                        file.flush().await?;
                    }
                    tracing::info!(
                        chunks,
                        bytes,
                        audio_chunks,
                        audio_bytes,
                        "recording chunk writer finished"
                    );
                    return Ok(());
                }
            }
        }

        file.flush().await?;
        if let Some(file) = audio_file.as_mut() {
            file.flush().await?;
        }
        tracing::info!(
            chunks,
            bytes,
            audio_chunks,
            audio_bytes,
            "recording chunk writer ended"
        );
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

  function gmrVisibleText(node) {
    if (!node) return '';
    const text = (node.innerText || node.textContent || '').replace(/\s+/g, ' ').trim();
    return text.slice(0, 160);
  }

  function gmrParticipantNameFromTile(tile, fallbackId) {
    const labelled = tile.querySelector('[aria-label]');
    const labels = [
      tile.getAttribute('aria-label') || '',
      labelled ? (labelled.getAttribute('aria-label') || '') : '',
      gmrVisibleText(tile)
    ].filter(Boolean);
    for (const raw of labels) {
      const cleaned = raw
        .replace(/\b(muted|speaking|presenting|camera is off|microphone is off)\b/ig, '')
        .replace(/\b(pin|more options|participant|tile)\b/ig, '')
        .replace(/\s+/g, ' ')
        .trim();
      if (cleaned && !/^\d+$/.test(cleaned)) return cleaned.slice(0, 120);
    }
    return fallbackId || 'Unknown participant';
  }

  function gmrExtractParticipants() {
    const byId = new Map();
    const tiles = Array.from(document.querySelectorAll('[data-participant-id], [data-requested-participant-id]'));
    for (const [index, tile] of tiles.entries()) {
      const rawId = tile.getAttribute('data-participant-id') || tile.getAttribute('data-requested-participant-id') || '';
      const name = gmrParticipantNameFromTile(tile, rawId || `tile:${index}`);
      const id = rawId || `name:${name.toLowerCase()}`;
      if (!byId.has(id)) {
        byId.set(id, {
          id,
          name,
          source: rawId ? 'participant_id' : 'tile_name',
          rawText: gmrVisibleText(tile),
          ariaLabel: tile.getAttribute('aria-label') || null
        });
      }
    }
    return Array.from(byId.values());
  }

  function gmrLooksLikeActiveSpeaker(tile) {
    const label = `${tile.getAttribute('aria-label') || ''} ${gmrVisibleText(tile)}`.toLowerCase();
    if (/\b(speaking|is speaking|active speaker)\b/.test(label)) return true;
    if (tile.querySelector('[aria-label*="speaking" i], [aria-label*="active speaker" i]')) return true;
    const className = typeof tile.className === 'string' ? tile.className.toLowerCase() : '';
    return /\b(active|speaking|speaker)\b/.test(className);
  }

  function gmrExtractActiveSpeaker(participants) {
    const tiles = Array.from(document.querySelectorAll('[data-participant-id], [data-requested-participant-id]'));
    for (const [index, tile] of tiles.entries()) {
      if (!gmrLooksLikeActiveSpeaker(tile)) continue;
      const rawId = tile.getAttribute('data-participant-id') || tile.getAttribute('data-requested-participant-id') || '';
      const name = gmrParticipantNameFromTile(tile, rawId || `tile:${index}`);
      const id = rawId || `name:${name.toLowerCase()}`;
      const participant = participants.find((p) => p.id === id);
      return {
        id,
        name: participant ? participant.name : name,
        source: rawId ? 'participant_id' : 'tile_name'
      };
    }
    return null;
  }

  function gmrCreateTimelineCollector(recordingStartedAtMs) {
    const state = {
      previousParticipants: new Map(),
      activeSpeakerId: null,
      snapshotCounter: 0
    };
    const makeEvent = (eventType, participant, payload) => {
      const nowMs = Date.now();
      return {
        eventType,
        occurredAt: Math.floor(nowMs / 1000),
        relativeMs: Math.max(0, nowMs - recordingStartedAtMs),
        participantId: participant && participant.id ? String(participant.id) : null,
        participantName: participant && participant.name ? String(participant.name) : null,
        payload: payload || {}
      };
    };
    return {
      collect() {
        const events = [];
        try {
          const participants = gmrExtractParticipants();
          const current = new Map(participants.map((participant) => [participant.id, participant]));
          for (const participant of participants) {
            if (!state.previousParticipants.has(participant.id)) {
              events.push(makeEvent('participant_joined', participant, { source: participant.source, rawText: participant.rawText, ariaLabel: participant.ariaLabel }));
            }
          }
          for (const [id, participant] of state.previousParticipants.entries()) {
            if (!current.has(id)) {
              events.push(makeEvent('participant_left', participant, { source: participant.source }));
            }
          }

          const activeSpeaker = gmrExtractActiveSpeaker(participants);
          const activeSpeakerId = activeSpeaker ? activeSpeaker.id : null;
          if (activeSpeakerId && activeSpeakerId !== state.activeSpeakerId) {
            events.push(makeEvent('active_speaker_changed', activeSpeaker, { source: activeSpeaker.source }));
          }
          state.activeSpeakerId = activeSpeakerId;

          state.snapshotCounter += 1;
          if (state.snapshotCounter === 1 || state.snapshotCounter % 6 === 0) {
            events.push(makeEvent('participant_snapshot', null, {
              count: participants.length,
              participants: participants.map((participant) => ({
                id: participant.id,
                name: participant.name,
                source: participant.source
              })),
              activeSpeakerId
            }));
          }

          state.previousParticipants = current;
        } catch (error) {
          events.push(makeEvent('collector_warning', null, {
            message: String(error && error.message ? error.message : error)
          }));
        }
        return events;
      }
    };
  }

  function gmrSendTimelineEvents(events) {
    if (!events || events.length === 0 || typeof window.__gmrRecordChunk !== 'function') return;
    const send = window.__gmrTimelinePending.then(async () => {
      window.__gmrRecordChunk(JSON.stringify({ kind: 'timeline_events', events }));
    }).catch((error) => {
      window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
    });
    window.__gmrTimelinePending = send;
    window.__gmrUploads.push(send);
  }

  window.__gmrRecordingStatus = { status: 'waiting_for_click' };
  if (window.__gmrRecorder && window.__gmrRecorder.state === 'recording') {
    window.__gmrRecordingStatus = {
      status: 'recording',
      mimeType: window.__gmrRecorder.mimeType || '',
      audioTrackCount: window.__gmrRecorderStream ? window.__gmrRecorderStream.getAudioTracks().length : null,
      videoTrackCount: window.__gmrRecorderStream ? window.__gmrRecorderStream.getVideoTracks().length : null,
      audioSource: window.__gmrAudioSource || null,
      streamInfo: window.__gmrStreamInfo || null
    };
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
    const displayMediaBase = {
      video: {
        displaySurface: 'browser',
        logicalSurface: true,
        cursor: 'always'
      },
      preferCurrentTab: true,
      selfBrowserSurface: 'include',
      surfaceSwitching: 'exclude'
    };
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({
        ...displayMediaBase,
        audio: false
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

    const videoTracks = stream.getVideoTracks();
    const audioWarnings = [];
    const meetElementAudioTracks = [];
    try {
      for (const [elementIndex, element] of Array.from(document.querySelectorAll('audio,video')).entries()) {
        const streamLike = element.srcObject && typeof element.srcObject.getAudioTracks === 'function'
          ? element.srcObject
          : (typeof element.captureStream === 'function' ? element.captureStream() : null);
        if (!streamLike || typeof streamLike.getAudioTracks !== 'function') {
          continue;
        }
        for (const [trackIndex, track] of streamLike.getAudioTracks().entries()) {
          if (track.readyState === 'live') {
            meetElementAudioTracks.push({ track, label: `meetElement:${elementIndex}:${trackIndex}` });
          }
        }
      }
    } catch (elementAudioError) {
      audioWarnings.push(`Meet element audio unavailable: ${elementAudioError && elementAudioError.message ? elementAudioError.message : String(elementAudioError)}`);
    }

    const audioInputs = meetElementAudioTracks;
    const audioInputTracks = audioInputs.map((input) => input.track);
    let audioTracks = audioInputTracks;
    const audioSource = audioInputs.length > 0 ? 'meet_element_audio' : 'none';
    let audioContext = null;
    let audioMixDestination = null;
    let audioMixSources = [];
    if (audioInputTracks.length > 0) {
      const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
      if (AudioContextCtor) {
        audioContext = new AudioContextCtor();
        if (audioContext.state === 'suspended') {
          await audioContext.resume();
        }
        audioMixDestination = audioContext.createMediaStreamDestination();
        audioMixSources = audioInputs.map((input) => {
          const source = audioContext.createMediaStreamSource(new MediaStream([input.track]));
          source.connect(audioMixDestination);
          return source;
        });
        audioTracks = audioMixDestination.stream.getAudioTracks();
      }
    }
    const recordingStream = new MediaStream([...videoTracks, ...audioTracks]);
    const mimeTypes = audioTracks.length > 0
      ? [
          'video/webm;codecs=vp8,opus',
          'video/webm;codecs=vp9,opus',
          'video/webm'
        ]
      : [
          'video/webm;codecs=vp8',
          'video/webm;codecs=vp9',
          'video/webm'
        ];
    const mimeType = mimeTypes.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '';
    const streamInfo = {
      audioSource,
      meetElementAudioTracks: meetElementAudioTracks.map(({ track, label }) => ({ label, trackLabel: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      audioTracks: audioTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      videoTracks: videoTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} }))
    };
    const recorder = new MediaRecorder(recordingStream, mimeType ? { mimeType } : undefined);
    const audioMimeTypes = [
      'audio/webm;codecs=opus',
      'audio/webm'
    ];
    const audioMimeType = audioTracks.length > 0
      ? (audioMimeTypes.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '')
      : '';
    const audioRecorder = audioTracks.length > 0
      ? new MediaRecorder(new MediaStream(audioTracks), audioMimeType ? { mimeType: audioMimeType } : undefined)
      : null;
    window.__gmrRecorder = recorder;
    window.__gmrAudioRecorder = audioRecorder;
    window.__gmrRecorderStream = recordingStream;
    window.__gmrCaptureStreams = [stream];
    window.__gmrAudioContext = audioContext;
    window.__gmrAudioMixDestination = audioMixDestination;
    window.__gmrAudioMixSources = audioMixSources;
    window.__gmrStreamInfo = streamInfo;
    window.__gmrAudioSource = audioSource;
    window.__gmrUploads = [];
    window.__gmrUploadErrors = [];
    for (const warning of audioWarnings) {
      window.__gmrUploadErrors.push(warning);
    }
    window.__gmrChunkCount = 0;
    window.__gmrAcceptedChunks = 0;
    window.__gmrUploadedBytes = 0;
    window.__gmrPendingUpload = Promise.resolve();
    window.__gmrPendingAudioUpload = Promise.resolve();
    window.__gmrTimelinePending = Promise.resolve();
    window.__gmrChunkBindingAvailable = typeof window.__gmrRecordChunk === 'function';
    if (audioTracks.length === 0) {
      window.__gmrUploadErrors.push('Meet media elements exposed no live audio tracks; recording video only');
    }

    window.__gmrTimelineCollector = gmrCreateTimelineCollector(Date.now());
    gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
    window.__gmrTimelineInterval = window.setInterval(() => {
      gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
    }, 5000);

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

    if (audioRecorder) {
      audioRecorder.ondataavailable = (event) => {
        if (!event.data || event.data.size === 0) return;
        const chunk = event.data;
        const upload = window.__gmrPendingAudioUpload.then(async () => {
          if (typeof window.__gmrRecordChunk !== 'function') {
            throw new Error('recording DevTools binding is unavailable');
          }
          const data = await blobToBase64(chunk);
          window.__gmrRecordChunk(JSON.stringify({
            kind: 'audio_chunk',
            size: chunk.size,
            data
          }));
        }).catch((error) => {
          window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
        });
        window.__gmrPendingAudioUpload = upload;
        window.__gmrUploads.push(upload);
      };
    }

    recorder.start(1000);
    if (audioRecorder) {
      audioRecorder.start(1000);
    }
    window.__gmrRecordingStatus = {
      status: 'recording',
      mimeType,
      streamInfo,
      audioTrackCount: audioTracks.length,
      videoTrackCount: videoTracks.length,
      audioSource
    };
  };

  return window.__gmrRecordingStatus;
}"#;

    let result: serde_json::Value = page.evaluate_function(script).await?.into_value()?;
    tracing::info!(%result, "installed browser recording control");
    Ok(())
}

async fn recording_status(page: &Page) -> anyhow::Result<RecordingStatus> {
    let value: serde_json::Value = page
        .evaluate_function(
            r#"function() {
  return window.__gmrRecordingStatus || { status: 'not_installed' };
}"#,
        )
        .await?
        .into_value()?;
    Ok(serde_json::from_value(value)?)
}

const MEET_PARTICIPANT_COUNT_SCRIPT: &str = r#"function() {
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
      const lower = body.toLowerCase();
      if (lower.includes("you're the only one here") || lower.includes('you\u2019re the only one here')) {
        out.count = 1;
        out.source = 'body_only_one';
        return out;
      }
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
}"#;

#[derive(Debug, serde::Deserialize)]
struct MeetParticipantSnapshot {
    count: Option<u64>,
}

pub async fn meet_participant_count(page: &Page) -> anyhow::Result<Option<u64>> {
    let value: serde_json::Value = page
        .evaluate_function(MEET_PARTICIPANT_COUNT_SCRIPT)
        .await?
        .into_value()?;
    let snapshot: MeetParticipantSnapshot = serde_json::from_value(value)?;
    Ok(snapshot.count)
}

pub async fn recording_runtime_status(page: &Page) -> anyhow::Result<RecordingRuntimeStatus> {
    let value: serde_json::Value = page
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
        .into_value()?;
    Ok(serde_json::from_value(value)?)
}

pub async fn stop_tab_recording(page: &Page) -> anyhow::Result<()> {
    let script = r#"async function() {
  const recorder = window.__gmrRecorder;
  if (!recorder) return { status: 'not_started', errors: [] };

  const stopRecorder = async (target) => {
    if (!target || target.state === 'inactive') return;
    await new Promise((resolve) => {
      const done = () => resolve();
      target.addEventListener('stop', done, { once: true });
      target.requestData();
      target.stop();
    });
  };

  await stopRecorder(window.__gmrAudioRecorder);
  await stopRecorder(recorder);

  if (window.__gmrRecorderStream) {
    window.__gmrRecorderStream.getTracks().forEach((track) => track.stop());
  }
  if (window.__gmrCaptureStreams) {
    window.__gmrCaptureStreams.forEach((stream) => stream.getTracks().forEach((track) => track.stop()));
  }
  if (window.__gmrTimelineInterval) {
    window.clearInterval(window.__gmrTimelineInterval);
    window.__gmrTimelineInterval = null;
  }
  if (window.__gmrTimelineCollector) {
    gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
  }
  if (window.__gmrAudioContext && window.__gmrAudioContext.state !== 'closed') {
    await window.__gmrAudioContext.close();
  }
  await Promise.allSettled(window.__gmrUploads || []);
  if (typeof window.__gmrRecordChunk === 'function') {
    window.__gmrRecordChunk(JSON.stringify({ kind: 'done' }));
  }
  return { status: 'stopped', errors: window.__gmrUploadErrors || [] };
}"#;

    let result: RecordingStatus = page.evaluate_function(script).await?.into_value()?;
    match result {
        RecordingStatus::Stopped { errors } | RecordingStatus::NotStarted { errors } => {
            if !errors.is_empty() {
                anyhow::bail!("browser upload errors: {errors:?}");
            }
        }
        other => {
            tracing::info!(?other, "browser MediaRecorder stopped");
        }
    }
    Ok(())
}
