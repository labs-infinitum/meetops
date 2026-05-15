use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
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

#[derive(Debug, Clone)]
pub struct JoinOptions {
    pub meet_url: String,
    pub display_name: String,
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub admission_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ProfileOptions {
    pub chromium_path: PathBuf,
    pub user_data_dir: PathBuf,
}

pub struct MeetingSession {
    browser: Browser,
    handler_task: JoinHandle<()>,
    pub page: Page,
}

impl MeetingSession {
    pub async fn close(mut self) {
        if tokio::time::timeout(Duration::from_secs(5), self.browser.close())
            .await
            .is_err()
        {
            tracing::warn!("timed out closing Chromium browser");
        }
        if tokio::time::timeout(Duration::from_secs(5), self.browser.wait())
            .await
            .is_err()
        {
            tracing::warn!("timed out waiting for Chromium browser process to exit");
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

    let (mut browser, mut handler) =
        Browser::launch(browser_config(&options.chromium_path, &user_data_dir)?).await?;
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
        }),
        Err(error) => {
            let _ = browser.close().await;
            let _ = browser.wait().await;
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

    let mut child = Command::new(&chrome_path)
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--profile-directory=Default")
        .arg("--new-window")
        .arg("--no-first-run")
        .arg("https://accounts.google.com/")
        .spawn()?;

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
        .arg("--auto-select-desktop-capture-source=Meet")
        .arg("--auto-select-tab-capture-source-by-title=Meet")
        .arg("--autoplay-policy=no-user-gesture-required")
        .arg("--mute-audio=false");

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

fn ensure_profile_dir(user_data_dir: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(user_data_dir)?;
    std::fs::write(user_data_dir.join(PROFILE_MARKER), b"bot profile\n")?;
    Ok(())
}

fn profile_dir(configured_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    ensure_profile_dir(configured_dir)?;
    Ok(configured_dir.canonicalize()?)
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
) -> anyhow::Result<BrowserRecording> {
    activate_page(page).await?;
    let recording = install_recording_binding(page, raw_webm_path.as_ref()).await?;
    install_recording_button(page).await?;
    let prompt_acceptor = spawn_tab_capture_prompt_acceptor();

    match page.find_element("#gmr-start-recording").await {
        Ok(button) => {
            if let Err(error) = button.click().await {
                tracing::warn!(%error, "could not click recorder start trigger");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "could not auto-click recorder start button");
        }
    }

    for _ in 0..120 {
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
            _ => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
        }
    }

    prompt_acceptor.abort();
    anyhow::bail!(
        "timed out waiting for browser recording to start; Chrome tab-capture approval may still be open"
    )
}

fn spawn_tab_capture_prompt_acceptor() -> JoinHandle<()> {
    tokio::spawn(async move {
        for attempt in 1..=10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Err(error) = accept_tab_capture_prompt().await {
                tracing::warn!(attempt, %error, "could not auto-accept Chrome tab capture prompt");
            } else {
                tracing::info!(
                    attempt,
                    "attempted to auto-accept Chrome tab capture prompt"
                );
            }
        }
    })
}

#[cfg(target_os = "macos")]
async fn accept_tab_capture_prompt() -> anyhow::Result<()> {
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

#[cfg(not(target_os = "macos"))]
async fn accept_tab_capture_prompt() -> anyhow::Result<()> {
    anyhow::bail!("automatic tab-capture prompt acceptance is currently implemented only on macOS")
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

                    file.write_all(&decoded).await?;
                    file.flush().await?;
                    chunks += 1;
                    bytes += decoded.len();
                    tracing::info!(chunks, bytes, "accepted recording chunk");
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
        window.__gmrRecordChunk(JSON.stringify({
          kind: 'chunk',
          sequence: window.__gmrChunkCount,
          size: chunk.size,
          data
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
    streamInfo: window.__gmrStreamInfo || null
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
