//! Linux X11 display discovery and optional virtual display (Xvfb) setup.
//!
//! When `[bot].x11_display` is unset, the bot probes displays (via `xdpyinfo`), prefers
//! high-numbered virtual displays (`:10+`), and starts `Xvfb` (+ optional `openbox`) if needed.

use std::{
    process::{Child, Stdio},
    time::Duration,
};

use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::config::BotConfig;

const X11_UNIX_DIR: &str = "/tmp/.X11-unix";
const XVFB_DISPLAY_MIN: u32 = 10;
const XVFB_DISPLAY_MAX: u32 = 99;
const XVFB_SCREEN: &str = "1920x1080x24";
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const XVFB_START_TIMEOUT: Duration = Duration::from_secs(15);

/// Resolved `DISPLAY` plus optional child processes to keep alive until drop.
pub struct X11Session {
    pub display: String,
    guard: Option<X11Guard>,
}

impl Drop for X11Session {
    fn drop(&mut self) {
        let _ = self.guard.take();
    }
}

struct X11Guard {
    xvfb: Option<Child>,
    openbox: Option<Child>,
}

impl Drop for X11Guard {
    fn drop(&mut self) {
        if let Some(child) = self.openbox.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(child) = self.xvfb.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Resolve `DISPLAY` for Chrome/xdotool and set it in the process environment.
#[cfg(target_os = "linux")]
pub async fn prepare(bot: &BotConfig) -> anyhow::Result<Option<X11Session>> {
    let (x11_display, guard) = resolve_display(bot).await?;
    // SAFETY: single-threaded setup before spawning Chrome/xdotool children.
    unsafe {
        std::env::set_var("DISPLAY", &x11_display);
    }
    info!(display = %x11_display, "using X11 DISPLAY for bot");
    Ok(Some(X11Session {
        display: x11_display,
        guard,
    }))
}

#[cfg(not(target_os = "linux"))]
pub async fn prepare(_bot: &BotConfig) -> anyhow::Result<Option<X11Session>> {
    Ok(None)
}

/// Canonical `DISPLAY` (e.g. `1` → `":1"`).
pub fn normalize_x11_display(raw: &str) -> String {
    let raw = raw.trim();
    let raw = raw.strip_prefix("DISPLAY=").map(str::trim).unwrap_or(raw);
    if raw.starts_with('/')
        || raw.contains("-unix/")
        || raw.starts_with('@')
        || raw.contains("unix:")
    {
        return raw.to_owned();
    }
    if raw.contains(':') && !raw.starts_with(':') {
        return raw.to_owned();
    }
    if raw.starts_with(':') {
        return raw.to_owned();
    }
    format!(":{raw}")
}

#[cfg(target_os = "linux")]
async fn resolve_display(bot: &BotConfig) -> anyhow::Result<(String, Option<X11Guard>)> {
    if let Some(configured) = bot.x11_display.as_deref() {
        let x11_display = normalize_x11_display(configured);
        ensure_reachable(&x11_display, "configured [bot].x11_display")?;
        let guard = spawn_openbox_guard(bot, &x11_display).await;
        return Ok((x11_display, guard));
    }

    if let Ok(env_display) = std::env::var("DISPLAY") {
        let x11_display = normalize_x11_display(&env_display);
        if display_probe(&x11_display).await {
            info!(display = %x11_display, source = "environment", "X11 display ready");
            let guard = spawn_openbox_guard(bot, &x11_display).await;
            return Ok((x11_display, guard));
        }
        warn!(
            display = %x11_display,
            "DISPLAY is set but not reachable; probing other displays or starting Xvfb"
        );
    }

    if !bot.x11_auto_virtual {
        anyhow::bail!(
            "Linux: no working X11 display.\n\
             Set [bot].x11_display, export a valid DISPLAY, or keep x11_auto_virtual enabled (default)."
        );
    }

    for x11_display in list_socket_displays_preferred() {
        if display_probe(&x11_display).await {
            info!(display = %x11_display, source = "discovered", "X11 display ready");
            let guard = spawn_openbox_guard(bot, &x11_display).await;
            return Ok((x11_display, guard));
        }
        if remove_stale_x11_socket(&x11_display) {
            warn!(
                display = %x11_display,
                "removed stale X11 socket left by a crashed Xvfb"
            );
            if display_probe(&x11_display).await {
                info!(display = %x11_display, source = "discovered_after_cleanup", "X11 display ready");
                let guard = spawn_openbox_guard(bot, &x11_display).await;
                return Ok((x11_display, guard));
            }
        }
        debug!(display = %x11_display, "X11 socket exists but display is not reachable");
    }

    start_virtual_display(bot).await
}

#[cfg(target_os = "linux")]
fn ensure_reachable(x11_display: &str, label: &str) -> anyhow::Result<()> {
    if display_probe_sync(x11_display) {
        info!(display = %x11_display, source = label, "X11 display ready");
        Ok(())
    } else {
        anyhow::bail!(
            "{label}={x11_display} is not reachable (xdpyinfo failed); \
             start Xvfb on that display or choose another"
        )
    }
}

#[cfg(target_os = "linux")]
async fn spawn_openbox_guard(bot: &BotConfig, x11_display: &str) -> Option<X11Guard> {
    if !bot.x11_start_openbox || openbox_running_on(x11_display).await {
        return None;
    }
    let openbox = spawn_openbox(x11_display)?;
    info!(display = %x11_display, "started openbox for xdotool window activation");
    Some(X11Guard {
        xvfb: None,
        openbox: Some(openbox),
    })
}

#[cfg(target_os = "linux")]
async fn start_virtual_display(bot: &BotConfig) -> anyhow::Result<(String, Option<X11Guard>)> {
    let xvfb_path = which_executable("Xvfb").ok_or_else(|| {
        anyhow::anyhow!(
            "no reachable X11 display and Xvfb not found in PATH; \
             install xvfb (e.g. apt install xvfb)"
        )
    })?;

    let n = pick_free_display_number().await?;
    let x11_display = format!(":{n}");

    remove_stale_x11_socket(&x11_display);

    info!(display = %x11_display, xvfb = %xvfb_path, "starting virtual X server (Xvfb)");
    let mut xvfb_child = std::process::Command::new(&xvfb_path)
        .arg(&x11_display)
        .args(["-screen", "0", XVFB_SCREEN, "-nolisten", "tcp", "-ac"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn Xvfb: {e}"))?;

    if !wait_display_ready(&x11_display, XVFB_START_TIMEOUT).await {
        let stderr = read_child_stderr(&mut xvfb_child);
        let _ = xvfb_child.kill();
        let _ = xvfb_child.wait();
        anyhow::bail!(
            "Xvfb on {x11_display} did not become ready in time. {stderr} \
             If the socket is stale: rm /tmp/.X11-unix/X{n}"
        );
    }

    info!(display = %x11_display, "Xvfb ready");

    let mut guard = X11Guard {
        xvfb: Some(xvfb_child),
        openbox: None,
    };
    if bot.x11_start_openbox
        && !openbox_running_on(&x11_display).await
        && let Some(openbox) = spawn_openbox(&x11_display)
    {
        info!(display = %x11_display, "started openbox for xdotool window activation");
        guard.openbox = Some(openbox);
    }

    Ok((x11_display, Some(guard)))
}

fn list_socket_displays_preferred() -> Vec<String> {
    let mut displays = list_socket_displays();
    displays.sort_by_key(|d| display_sort_key(d));
    displays
}

fn list_socket_displays() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(X11_UNIX_DIR) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(n) = name.strip_prefix('X').and_then(|s| s.parse::<u32>().ok()) {
            out.push(format!(":{n}"));
        }
    }
    out
}

/// Prefer bot-owned virtual displays (`:10+`), then other non-zero, then `:0`.
fn display_sort_key(display: &str) -> (u8, u32) {
    let n = display_number(display);
    if n == 0 {
        (2, 0)
    } else if (XVFB_DISPLAY_MIN..=XVFB_DISPLAY_MAX).contains(&n) {
        (0, n)
    } else {
        (1, n)
    }
}

fn display_number(display: &str) -> u32 {
    let d = normalize_x11_display(display);
    d.strip_prefix(':')
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
async fn pick_free_display_number() -> anyhow::Result<u32> {
    for n in XVFB_DISPLAY_MIN..=XVFB_DISPLAY_MAX {
        let display = format!(":{n}");
        if display_probe(&display).await {
            continue;
        }
        if x11_socket_path(&display).is_some() {
            remove_stale_x11_socket(&display);
        }
        return Ok(n);
    }
    anyhow::bail!("no free X11 display slot in :{XVFB_DISPLAY_MIN}..:{XVFB_DISPLAY_MAX}");
}

/// Remove a leftover `/tmp/.X11-unix/X{n}` when no server answers on that display.
#[cfg(target_os = "linux")]
fn remove_stale_x11_socket(display: &str) -> bool {
    let Some(path) = x11_socket_path(display) else {
        return false;
    };
    if display_probe_sync(display) {
        return false;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => true,
        Err(error) => {
            let x11 = display;
            warn!(x11_display = %x11, socket_path = ?path, %error, "could not remove stale X11 socket");
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn x11_socket_path(display: &str) -> Option<std::path::PathBuf> {
    let n = display_number(display);
    let path = std::path::Path::new(X11_UNIX_DIR).join(format!("X{n}"));
    path.exists().then_some(path)
}

#[cfg(target_os = "linux")]
async fn display_probe(display: &str) -> bool {
    tokio::time::timeout(PROBE_TIMEOUT, async {
        tokio::process::Command::new("xdpyinfo")
            .env("DISPLAY", display)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .ok()
            .map(|s| s.success())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn display_probe_sync(display: &str) -> bool {
    std::process::Command::new("xdpyinfo")
        .env("DISPLAY", display)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
async fn wait_display_ready(display: &str, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if display_probe(display).await {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(target_os = "linux")]
async fn openbox_running_on(display: &str) -> bool {
    let Ok(output) = tokio::process::Command::new("pgrep")
        .args(["-af", "openbox"])
        .output()
        .await
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .any(|line| line.contains(&format!("DISPLAY={display}")) || line.contains(display))
}

#[cfg(target_os = "linux")]
fn spawn_openbox(display: &str) -> Option<Child> {
    let openbox = which_executable("openbox")?;
    std::process::Command::new(openbox)
        .env("DISPLAY", display)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn which_executable(name: &str) -> Option<String> {
    let output = std::process::Command::new("which")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if path.is_empty() { None } else { Some(path) }
}

fn read_child_stderr(child: &mut Child) -> String {
    use std::io::Read;
    let mut buf = String::new();
    if let Some(stderr) = child.stderr.take() {
        if let Err(error) = std::io::BufReader::new(stderr).read_to_string(&mut buf) {
            tracing::debug!(%error, "could not read child stderr for error context");
        }
    }
    if buf.trim().is_empty() {
        String::new()
    } else {
        format!("stderr: {}", buf.trim())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn normalize_display_numbers() {
        assert_eq!(normalize_x11_display("1"), ":1");
        assert_eq!(normalize_x11_display(":1"), ":1");
        assert_eq!(normalize_x11_display("DISPLAY=:2"), ":2");
    }

    #[test]
    fn display_sort_prefers_high_virtual() {
        let mut v = vec![
            ":0".to_owned(),
            ":1".to_owned(),
            ":12".to_owned(),
            ":99".to_owned(),
        ];
        v.sort_by_key(|d| display_sort_key(d));
        assert_eq!(v[0], ":12");
        assert_eq!(v[1], ":99");
        assert_eq!(v[2], ":1");
        assert_eq!(v[3], ":0");
    }

    #[test]
    fn list_socket_displays_reads_x11_unix_dir() {
        if Path::new(X11_UNIX_DIR).is_dir() {
            let _ = list_socket_displays();
        }
    }
}
