//! Linux-only: wait after `getDisplayMedia`, screenshot the Chrome window via X11 tooling, locate
//! the pale-blue \"Allow\" pill with [`allow_button_detect`], then fall back to heuristic `xdotool`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::bot::{allow_button_detect, xdotool_geom};
use anyhow::Context;
use tokio::process::Command;
use tokio::sync::oneshot::Receiver;
use tokio::task::JoinHandle;

use super::{
    linux_accept_tab_capture_prompt, linux_find_largest_chromium_window,
    linux_xdotool_activate_window, linux_xdotool_apply_display,
};

const WAIT_AFTER_CAPTURE_REQUEST: Duration = Duration::from_secs(3);
const SCREENSHOT_RETRY_INTERVAL: Duration = Duration::from_millis(400);
const SCREENSHOT_ATTEMPTS: usize = 14;

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(30);
const XDOTOOL_SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(5);

async fn maybe_dismiss_restore_pages_from_png(
    shot_path: &Path,
    wid_str: &str,
    display: Option<&str>,
) -> anyhow::Result<bool> {
    let path_owned = shot_path.to_path_buf();
    let center = tokio::task::spawn_blocking(move || {
        let img = image::open(&path_owned).ok()?;
        let popup = allow_button_detect::detect_restore_pages_popup(&img)?;
        Some(popup.close_button_center())
    })
    .await
    .map_err(|_| anyhow::anyhow!("Restore pages blob task aborted"))?;

    let Some((cx_u, cy_u)) = center else {
        tracing::trace!(
            screenshot_png = %shot_path.display(),
            "Restore pages blob: no top-right title card matched"
        );
        return Ok(false);
    };

    let cx = i32::try_from(cx_u).context("Restore close x overflows i32")?;
    let cy = i32::try_from(cy_u).context("Restore close y overflows i32")?;

    tracing::info!(
        screenshot_png = %shot_path.display(),
        xdotool_window_id = %wid_str,
        center_x_window_relative = cx,
        center_y_window_relative = cy,
        method = "restore_pages_blob",
        "Restore pages? close (×) from PNG title-card blob",
    );

    xdot_mousemove_click_window_relative(wid_str, cx, cy, display).await?;
    Ok(true)
}

async fn maybe_click_allow_from_color_blob_png(
    shot_path: &Path,
    wid_str: &str,
    display: Option<&str>,
) -> anyhow::Result<bool> {
    let path_owned = shot_path.to_path_buf();
    let candidate = tokio::task::spawn_blocking(move || match image::open(&path_owned) {
        Ok(img) => allow_button_detect::detect_tab_share_allow_button(&img),
        Err(error) => {
            tracing::warn!(
                screenshot_png = %path_owned.display(),
                %error,
                "Allow color-blob detector could not open PNG"
            );
            None
        }
    })
    .await;

    let candidate = candidate.map_err(|_| anyhow::anyhow!("Allow color blob task aborted"))?;

    let Some(button) = candidate else {
        tracing::trace!(
            screenshot_png = %shot_path.display(),
            "Allow color-blob: no pale-blue pill matched"
        );
        return Ok(false);
    };

    let (cx_u, cy_u) = button.center();
    let cx = i32::try_from(cx_u).context("Allow center x overflows i32")?;
    let cy = i32::try_from(cy_u).context("Allow center y overflows i32")?;

    tracing::info!(
        screenshot_png = %shot_path.display(),
        xdotool_window_id = %wid_str,
        bbox_min_x = button.min_x,
        bbox_min_y = button.min_y,
        bbox_max_x = button.max_x,
        bbox_max_y = button.max_y,
        center_x_window_relative = cx,
        center_y_window_relative = cy,
        method = "color_blob",
        "Allow pose from PNG chroma+flood-fill (window-relative for xdotool --window)",
    );

    xdot_mousemove_click_window_relative(wid_str, cx, cy, display).await?;
    Ok(true)
}

pub(super) fn spawn(
    wait_for_click: Receiver<()>,
    linux_xdotool_display: Option<String>,
    tab_capture_screenshot_dir: PathBuf,
    tab_capture_png: PathBuf,
    launched_chrome_pid: Option<u32>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let x11_display_opt = linux_xdotool_display.as_deref();
        if wait_for_click.await.is_err() {
            tracing::debug!(
                "tab capture auto-accept not started (recorder trigger click skipped or canceled)"
            );
            return;
        }

        tracing::info!(
            x11_display_explicit = ?x11_display_opt,
            process_display_env = ?std::env::var("DISPLAY").ok(),
            launched_chrome_pid = ?launched_chrome_pid,
            tab_capture_shots_dir = %tab_capture_screenshot_dir.display(),
            tab_capture_png = %tab_capture_png.display(),
            "Linux tab-share auto-accept: dismiss restore bubble, 3s pause — PNG color blob, heuristic xdotool"
        );

        tokio::time::sleep(WAIT_AFTER_CAPTURE_REQUEST).await;

        if let Err(error) = tokio::fs::create_dir_all(tab_capture_screenshot_dir.as_path()).await {
            tracing::warn!(
                %error,
                dir = %tab_capture_screenshot_dir.display(),
                "could not create tab-capture screenshots directory under recording data dir"
            );
        }

        for attempt in 1..=SCREENSHOT_ATTEMPTS {
            match attempt_click_allow_via_screenshot(
                x11_display_opt,
                tab_capture_screenshot_dir.as_path(),
                tab_capture_png.as_path(),
                launched_chrome_pid,
            )
            .await
            {
                Ok(()) => {
                    tracing::info!(attempt, "tab-capture prompt detection pass finished");
                }
                Err(error) => {
                    tracing::warn!(
                        attempt,
                        %error,
                        "screenshot Allow click attempt failed"
                    );
                }
            }
            tokio::time::sleep(SCREENSHOT_RETRY_INTERVAL).await;
        }

        for attempt in 1..=48 {
            if let Err(error) =
                linux_accept_tab_capture_prompt(x11_display_opt, launched_chrome_pid).await
            {
                tracing::warn!(attempt, %error, "could not auto-accept Chrome tab capture prompt");
            } else {
                tracing::trace!(attempt, "heuristic tab-capture infra pass finished");
            }
            tokio::time::sleep(Duration::from_millis(85)).await;
        }
    })
}

async fn attempt_click_allow_via_screenshot(
    display: Option<&str>,
    screenshot_dir: &Path,
    tab_capture_png: &Path,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<()> {
    let (shot_path, wid_str) = chrome_window_focus_and_capture_png(
        display,
        screenshot_dir,
        "chrome-tab-share-match",
        launched_chrome_pid,
    )
    .await?;

    persist_tab_capture_png_copy(shot_path.as_path(), tab_capture_png).await?;

    tracing::info!(
        screenshot_png = %shot_path.display(),
        tab_capture_png = %tab_capture_png.display(),
        xdotool_window_id = %wid_str,
        "saved Chrome window screenshot for tab-share Allow detection",
    );

    if maybe_dismiss_restore_pages_from_png(shot_path.as_path(), &wid_str, display).await? {
        tracing::info!(
            screenshot_png = %shot_path.display(),
            xdotool_window_id = %wid_str,
            "Dismissed Restore pages? via blob detector before Allow search",
        );
        tokio::time::sleep(Duration::from_millis(350)).await;
        return Ok(());
    }

    if maybe_click_allow_from_color_blob_png(shot_path.as_path(), &wid_str, display).await? {
        tracing::info!(
            screenshot_png = %shot_path.display(),
            xdotool_window_id = %wid_str,
            "Allow dismissed via color blob detector on screenshot",
        );
        return Ok(());
    }

    tracing::debug!(
        screenshot_png = %shot_path.display(),
        "Allow color blob missed"
    );
    Ok(())
}

/// Focus Google Chrome via `xdotool`, then save the window pixmap as PNG.
async fn chrome_window_focus_and_capture_png(
    display: Option<&str>,
    screenshot_dir: &Path,
    filename_stem: &str,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<(PathBuf, String)> {
    let wid = tokio::time::timeout(
        XDOTOOL_SUBPROCESS_TIMEOUT,
        linux_find_largest_chromium_window(display, launched_chrome_pid),
    )
    .await
    .map_err(|_| anyhow::anyhow!("xdotool search timed out"))??;
    let wid_str = wid.to_string();

    linux_xdotool_activate_window(XDOTOOL_SUBPROCESS_TIMEOUT, display, &wid_str).await?;

    tokio::time::sleep(Duration::from_millis(220)).await;

    let shot_path = screenshot_dir.join(format!(
        "{}-{}.png",
        filename_stem,
        unique_timestamp_suffix()
    ));

    screenshot_chrome_window_to_png(wid, display, shot_path.as_path()).await?;

    Ok((shot_path, wid_str))
}

async fn persist_tab_capture_png_copy(
    shot_path: &Path,
    tab_capture_png: &Path,
) -> anyhow::Result<()> {
    if let Some(parent) = tab_capture_png.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(shot_path, tab_capture_png).await?;
    Ok(())
}

fn unique_timestamp_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("{}", d.as_nanos()))
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "system clock before UNIX epoch; using fallback screenshot suffix");
            "0".into()
        })
}

async fn screenshot_chrome_window_to_png(
    wid: u32,
    display: Option<&str>,
    out: &Path,
) -> anyhow::Result<()> {
    let wid_s = wid.to_string();

    if screenshot_try_magick(&wid_s, display, out).await? {
        return Ok(());
    }
    if screenshot_try_import(&wid_s, display, out).await? {
        return Ok(());
    }
    if screenshot_try_maim(&wid_s, display, out).await? {
        return Ok(());
    }

    ffmpeg_x11grab_window(wid, display, out).await
}

async fn screenshot_try_magick(
    wid_str: &str,
    display: Option<&str>,
    out: &Path,
) -> anyhow::Result<bool> {
    let mut cmd = Command::new("magick");
    linux_xdotool_apply_display(&mut cmd, display);
    cmd.args(["import", "-silent", "-window", wid_str, "-format", "png"])
        .arg(out);
    run_optional_tool(cmd).await
}

async fn screenshot_try_import(
    wid_str: &str,
    display: Option<&str>,
    out: &Path,
) -> anyhow::Result<bool> {
    let mut cmd = Command::new("import");
    linux_xdotool_apply_display(&mut cmd, display);
    cmd.args(["-silent", "-window", wid_str, "-format", "png"])
        .arg(out);
    run_optional_tool(cmd).await
}

async fn screenshot_try_maim(
    wid_str: &str,
    display: Option<&str>,
    out: &Path,
) -> anyhow::Result<bool> {
    let mut cmd = Command::new("maim");
    linux_xdotool_apply_display(&mut cmd, display);
    cmd.args(["-i", wid_str, "-f", "png"]).arg(out);
    run_optional_tool(cmd).await
}

/// Ok(true)=success wrote file, Ok(false)=tool absent, Err=failed.
async fn run_optional_tool(mut cmd: Command) -> anyhow::Result<bool> {
    match tokio::time::timeout(SCREENSHOT_TIMEOUT, cmd.status()).await {
        Ok(Ok(st)) => {
            if st.success() {
                Ok(true)
            } else {
                let code = st.code();
                if code == Some(127) || code == Some(133) {
                    Ok(false)
                } else {
                    anyhow::bail!("screenshot helper exited with status {:?}", code);
                }
            }
        }
        Ok(Err(e)) if e.kind() == ErrorKind::NotFound => Ok(false),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => anyhow::bail!("screenshot helper timed out after {:?}", SCREENSHOT_TIMEOUT),
    }
}

async fn ffmpeg_x11grab_window(wid: u32, display: Option<&str>, out: &Path) -> anyhow::Result<()> {
    let geo = grab_window_geom(wid, display).await?;

    let x = geo
        .x
        .context(
            "ffmpeg fallback needs X from xdotool getwindowgeometry — install magick/import/maim for -window grabs",
        )?
        .max(0);
    let y = geo
        .y
        .context("ffmpeg fallback needs Y from xdotool getwindowgeometry")?
        .max(0);
    let w = geo.width.and_then(|n| if n > 0 { Some(n as u32) } else { None }).context(
        "ffmpeg fallback needs WIDTH > 0 from xdotool getwindowgeometry — install magick/import/maim for -window grabs",
    )?;
    let h = geo
        .height
        .and_then(|n| if n > 0 { Some(n as u32) } else { None })
        .context("ffmpeg fallback needs HEIGHT > 0")?;

    let mut ff_disp = display
        .map(str::to_string)
        .context("DISPLAY is unset; x11_display::prepare should set it before tab capture")?;

    // Simple ":N" → ":N.0" style for ffmpeg x11grab single-screen notation.
    if ff_disp.starts_with(':')
        && !ff_disp.contains('/')
        && !ff_disp.contains('@')
        && !ff_disp.contains("unix:")
        && !ff_disp.chars().any(|c| c == '.')
    {
        ff_disp.push_str(".0");
    }

    let vid = format!("{ff_disp}+{x},{y}");

    let mut cmd = Command::new("ffmpeg");
    linux_xdotool_apply_display(&mut cmd, display);
    cmd.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "x11grab",
        "-video_size",
        &format!("{w}x{h}"),
    ]);
    cmd.arg("-i").arg(&vid);
    cmd.arg("-frames:v").arg("1");
    cmd.arg(out);
    match tokio::time::timeout(SCREENSHOT_TIMEOUT, cmd.status()).await {
        Ok(Ok(st)) if st.success() => Ok(()),
        Ok(Ok(st)) => anyhow::bail!("ffmpeg x11grab failed with status {st:?}"),
        Ok(Err(e)) if e.kind() == ErrorKind::NotFound => {
            anyhow::bail!(
                "no magick/import/maim/ffmpeg on PATH — install ImageMagick (`import`), `maim`, or ffmpeg for screenshots"
            );
        }
        Ok(Err(e)) => Err(anyhow::Error::new(e)),
        Err(_) => anyhow::bail!("ffmpeg x11grab timed out"),
    }
}

async fn grab_window_geom(
    wid: u32,
    display: Option<&str>,
) -> anyhow::Result<xdotool_geom::ShellGeom> {
    let wid_s = wid.to_string();
    let out = tokio::time::timeout(XDOTOOL_SUBPROCESS_TIMEOUT, {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["getwindowgeometry", "--shell", &wid_s]).output()
    })
    .await??;

    anyhow::ensure!(out.status.success(), "getwindowgeometry failed");
    Ok(xdotool_geom::parse_geom_shell_stdout(
        &String::from_utf8_lossy(&out.stdout),
    ))
}

async fn xdot_mousemove_click_window_relative(
    wid_str: &str,
    cx: i32,
    cy: i32,
    display: Option<&str>,
) -> anyhow::Result<()> {
    let mut mv = Command::new("xdotool");
    linux_xdotool_apply_display(&mut mv, display);
    mv.args([
        "mousemove",
        "--window",
        wid_str,
        &cx.to_string(),
        &cy.to_string(),
    ]);
    anyhow::ensure!(
        tokio::time::timeout(XDOTOOL_SUBPROCESS_TIMEOUT, mv.status())
            .await??
            .success(),
        "xdotool mousemove --window failed"
    );

    tokio::time::sleep(Duration::from_millis(35)).await;

    let mut clk = Command::new("xdotool");
    linux_xdotool_apply_display(&mut clk, display);
    clk.args(["click", "1"]);

    anyhow::ensure!(
        tokio::time::timeout(XDOTOOL_SUBPROCESS_TIMEOUT, clk.status())
            .await??
            .success(),
        "xdotool click failed"
    );

    Ok(())
}
