use std::{path::PathBuf, time::Duration};

use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::Instant;

const PROFILE_MARKER: &str = "google-meeting-recorder-bot-profile";

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

    if result.is_err() {
        let _ = browser.close().await;
        let _ = browser.wait().await;
        handler_task.abort();
    }

    result
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
        .window_size(1280, 720)
        .arg("--profile-directory=Default")
        .arg("--new-window")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-infobars")
        .arg("--disable-notifications")
        .arg("--use-fake-ui-for-media-stream")
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

async fn run_join_flow(browser: &Browser, options: &JoinOptions) -> anyhow::Result<()> {
    tracing::info!(url = %options.meet_url, "opening Google Meet");
    let page = browser.new_page(options.meet_url.as_str()).await?;
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
                tracing::info!("bot is in the meeting; press Ctrl+C to leave and close Chromium");
                tokio::signal::ctrl_c().await?;
                return Ok(());
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
