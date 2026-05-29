//! Linux-only: wait after `getDisplayMedia`, screenshot the Chrome window via X11 tooling, then locate
//! the pale-blue \"Allow\" pill with [`allow_button_detect`] on that PNG and click; optionally fall back
//! to `allow_button_template_png` grayscale template correlation when the blob misses, then heuristic `xdotool`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::bot::allow_button_detect;
use anyhow::Context;
use image::GrayImage;
use tokio::process::Command;
use tokio::sync::oneshot::Receiver;
use tokio::task::JoinHandle;

use super::{
    linux_accept_tab_capture_prompt, linux_find_largest_chromium_window,
    linux_xdotool_activate_window, linux_xdotool_apply_display,
};

const WAIT_AFTER_CAPTURE_REQUEST: Duration = Duration::from_secs(3);
const TEMPLATE_RETRY_INTERVAL: Duration = Duration::from_millis(400);
const TEMPLATE_ATTEMPTS: usize = 14;
/// Normalized-cross-correlation score threshold for template refinement (similar idea to tuning OpenCV TM_CCOEFF_NORMED).
const MIN_TEMPLATE_MATCH_SCORE: f64 = 0.72;

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(30);
const XDOTOOL_SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(5);

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

    println!(
        "[gmr-tab-capture] Allow from color blob (window-relative):\n\
  png: {}\n\
  window: {}\n\
  bbox x=[{}…{}] y=[{}…{}]\n\
  click_center=({}, {})",
        shot_path.display(),
        wid_str,
        button.min_x,
        button.max_x,
        button.min_y,
        button.max_y,
        cx,
        cy,
    );

    xdot_mousemove_click_window_relative(wid_str, cx, cy, display).await?;
    Ok(true)
}

pub(super) fn spawn(
    wait_for_click: Receiver<()>,
    linux_xdotool_display: Option<String>,
    allow_button_template_png: Option<PathBuf>,
    tab_capture_screenshot_dir: PathBuf,
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
            button_template_png = ?allow_button_template_png,
            launched_chrome_pid = ?launched_chrome_pid,
            tab_capture_shots_dir = %tab_capture_screenshot_dir.display(),
            "Linux tab-share auto-accept: 3s pause after capture request — PNG color blob, optional template refinement, heuristic xdotool"
        );

        tokio::time::sleep(WAIT_AFTER_CAPTURE_REQUEST).await;

        if let Err(error) = tokio::fs::create_dir_all(tab_capture_screenshot_dir.as_path()).await {
            tracing::warn!(
                %error,
                dir = %tab_capture_screenshot_dir.display(),
                "could not create tab-capture screenshots directory under recording data dir"
            );
        }

        let template_path = resolve_template_path_with_warning(allow_button_template_png.as_ref());

        match template_path {
            Some(ref path) => {
                for attempt in 1..=TEMPLATE_ATTEMPTS {
                    match attempt_click_allow_via_screenshot(
                        x11_display_opt,
                        path.as_path(),
                        tab_capture_screenshot_dir.as_path(),
                        launched_chrome_pid,
                    )
                    .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                attempt,
                                template = %path.display(),
                                "Allow click attempted (screenshot + blob or template refinement)"
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                attempt,
                                %error,
                                "screenshot/template Allow click attempt failed"
                            );
                        }
                    }
                    tokio::time::sleep(TEMPLATE_RETRY_INTERVAL).await;
                }

                tracing::warn!(
                    "template-assisted Allow retries exhausted; falling back to heuristic xdotool"
                );
            }
            None => {
                tracing::info!(
                    "allow_button_template_png not set — capturing one Chrome window PNG (color blob only), then heuristic xdotool; set template path for grayscale fallback",
                );
                match chrome_window_focus_and_capture_png(
                    x11_display_opt,
                    tab_capture_screenshot_dir.as_path(),
                    "chrome-tab-share-no-template",
                    launched_chrome_pid,
                )
                .await
                {
                    Ok((path, wid)) => {
                        match maybe_click_allow_from_color_blob_png(
                            path.as_path(),
                            &wid,
                            x11_display_opt,
                        )
                        .await
                        {
                            Ok(true) => {
                                tracing::info!(
                                    screenshot_png = %path.display(),
                                    window_id = %wid,
                                    "clicked Allow via color blob (allow_button_template_png not set)",
                                );
                                println!(
                                    "[gmr-tab-capture] Allow clicked via color blob; png={} window={}",
                                    path.display(),
                                    wid,
                                );
                            }
                            Ok(false) => {
                                tracing::info!(
                                    screenshot_png = %path.display(),
                                    dir = %tab_capture_screenshot_dir.display(),
                                    window_id = %wid,
                                    "saved debug Chrome screenshot — color blob did not find Allow pill",
                                );
                                println!(
                                    "[gmr-tab-capture] screenshot saved — Allow not auto-clicked (blob miss). png={} window={} — set allow_button_template_png or run: cargo run --bin find-chrome-tab-allow-click -- <png> --window-id <id>",
                                    path.display(),
                                    wid,
                                );
                            }
                            Err(error) => {
                                tracing::warn!(
                                    screenshot_png = %path.display(),
                                    window_id = %wid,
                                    %error,
                                    "color blob detector or xdotool click failed — saved screenshot under recording dir",
                                );
                                tracing::info!(
                                    screenshot_png = %path.display(),
                                    dir = %tab_capture_screenshot_dir.display(),
                                    window_id = %wid,
                                    "saved debug Chrome screenshot after failed blob click attempt",
                                );
                                println!(
                                    "[gmr-tab-capture] screenshot saved — blob click attempt failed (see logs). png={} window={}",
                                    path.display(),
                                    wid,
                                );
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            dir = %tab_capture_screenshot_dir.display(),
                            "could not capture debug Chrome PNG; verify xdotool, DISPLAY=:1 matching Chrome, imagemagick import/maim or ffmpeg x11grab"
                        );
                    }
                }
            }
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

fn resolve_template_path_with_warning(candidate: Option<&PathBuf>) -> Option<PathBuf> {
    let p = candidate?.clone();
    if p.is_file() {
        Some(p)
    } else {
        tracing::warn!(
            path = %p.display(),
            "allow_button_template_png missing or not a file; disabling template match"
        );
        None
    }
}

#[derive(Debug)]
struct TemplateMatch {
    x: u32,
    y: u32,
    tw: u32,
    th: u32,
    score: f64,
}

async fn attempt_click_allow_via_screenshot(
    display: Option<&str>,
    template_png: &Path,
    screenshot_dir: &Path,
    launched_chrome_pid: Option<u32>,
) -> anyhow::Result<()> {
    let (shot_path, wid_str) = chrome_window_focus_and_capture_png(
        display,
        screenshot_dir,
        "chrome-tab-share-match",
        launched_chrome_pid,
    )
    .await?;

    tracing::info!(
        screenshot_png = %shot_path.display(),
        xdotool_window_id = %wid_str,
        "saved Chrome window screenshot for tab-share Allow detection",
    );

    if maybe_click_allow_from_color_blob_png(shot_path.as_path(), &wid_str, display).await? {
        tracing::info!(
            screenshot_png = %shot_path.display(),
            xdotool_window_id = %wid_str,
            skipped = "template_png",
            "Allow dismissed via color blob detector on screenshot",
        );
        return Ok(());
    }

    tracing::debug!(
        screenshot_png = %shot_path.display(),
        template = %template_png.display(),
        "color blob missed — correlating Allow template PNG",
    );

    let tmpl_path_owned = template_png.to_path_buf();
    let screenshot_path_for_match = shot_path.clone();
    let m = tokio::task::spawn_blocking(move || {
        let scene_dyn = image::open(&screenshot_path_for_match).with_context(|| {
            format!(
                "read screenshot PNG {}",
                screenshot_path_for_match.display()
            )
        })?;
        let tmpl_dyn = image::open(&tmpl_path_owned)
            .with_context(|| format!("{}", tmpl_path_owned.display()))?;
        let scene = scene_dyn.into_luma8();
        let tmpl = tmpl_dyn.into_luma8();
        let m = match_template_ncc(&scene, &tmpl)?;
        anyhow::ensure!(
            m.score >= MIN_TEMPLATE_MATCH_SCORE,
            "weak match {:.3}",
            m.score
        );
        Ok::<TemplateMatch, anyhow::Error>(m)
    })
    .await??;

    let cx = i32::try_from(m.x + m.tw / 2)?;
    let cy = i32::try_from(m.y + m.th / 2)?;

    tracing::info!(
        screenshot_png = %shot_path.display(),
        xdotool_window_id = %wid_str,
        bbox_x = m.x,
        bbox_y = m.y,
        bbox_w = m.tw,
        bbox_h = m.th,
        center_x_window_relative = cx,
        center_y_window_relative = cy,
        match_score_ncc = %format!("{:.5}", m.score),
        "Allow button pose from screenshot + template (window-relative coords; pass to xdotool --window)",
    );

    println!(
        "[gmr-tab-capture] Allow button pose after screenshot:\n\
  png: {}\n\
  window: {} — use coords with `xdotool mousemove --window <id> x y`\n\
  bbox top_left=({}, {}) size {}x{}\n\
  click_center=({}, {})  template_corr={:.5}",
        shot_path.display(),
        wid_str,
        m.x,
        m.y,
        m.tw,
        m.th,
        cx,
        cy,
        m.score,
    );

    xdot_mousemove_click_window_relative(&wid_str, cx, cy, display).await?;

    Ok(())
}

/// Focus Chrome/Chromium/Brave via `xdotool`, then save the window pixmap as PNG.
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

fn unique_timestamp_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("{}", d.as_nanos()))
        .unwrap_or_else(|_| "0".into())
}

fn match_template_ncc(scene: &GrayImage, tpl: &GrayImage) -> anyhow::Result<TemplateMatch> {
    let sw = scene.width();
    let sh = scene.height();
    let tw = tpl.width();
    let th = tpl.height();
    anyhow::ensure!(
        tw <= sw && th <= sh,
        "template exceeds screenshot dimensions"
    );

    let area = tw * th;
    let n = f64::from(area);

    let mut t_dc: Vec<f64> = Vec::with_capacity(area as usize);
    let mut sum_t = 0.0_f64;
    for y in 0..th {
        for x in 0..tw {
            let v = f64::from(tpl.get_pixel(x, y).0[0]);
            sum_t += v;
        }
    }
    let mean_t = sum_t / n;

    let mut t_norm_sq = 0.0_f64;
    for y in 0..th {
        for x in 0..tw {
            let v = f64::from(tpl.get_pixel(x, y).0[0]);
            let d = v - mean_t;
            t_dc.push(d);
            t_norm_sq += d * d;
        }
    }
    anyhow::ensure!(t_norm_sq > 1e-9, "degenerate flat template");

    let max_sy = sh - th;
    let max_sx = sw - tw;
    let mut best_score = f64::NEG_INFINITY;
    let mut bx = 0u32;
    let mut by = 0u32;
    let mut matched_any = false;

    for sy in 0..=max_sy {
        for sx in 0..=max_sx {
            let mut sum_p = 0.0_f64;
            let mut scratch: Vec<f64> = Vec::with_capacity(area as usize);
            for y in 0..th {
                for x in 0..tw {
                    let v = f64::from(scene.get_pixel(sx + x, sy + y).0[0]);
                    scratch.push(v);
                    sum_p += v;
                }
            }
            let mean_p = sum_p / n;

            let mut dot = 0.0_f64;
            let mut p_norm_sq = 0.0_f64;

            if t_dc.len() != scratch.len() {
                anyhow::bail!("internal template/patch length mismatch");
            }

            for (t_d, p_raw) in t_dc.iter().zip(scratch.iter()) {
                let p_d = *p_raw - mean_p;
                dot += (*t_d) * p_d;
                p_norm_sq += p_d * p_d;
            }

            if p_norm_sq < 1e-9 {
                continue;
            }
            let denom = (t_norm_sq * p_norm_sq).sqrt();
            if denom < 1e-18 {
                continue;
            }
            let score = dot / denom;
            if score > best_score {
                best_score = score;
                bx = sx;
                by = sy;
                matched_any = true;
            }
        }
    }

    anyhow::ensure!(
        matched_any && best_score.is_finite(),
        "ncc had no usable match"
    );
    Ok(TemplateMatch {
        x: bx,
        y: by,
        tw,
        th,
        score: best_score,
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
        .or_else(|| std::env::var("DISPLAY").ok())
        .context(
            "DISPLAY is unset; pass [bot].x11_display or export DISPLAY for ffmpeg fallback",
        )?;

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

#[derive(Default)]
struct ShellGeom {
    x: Option<i32>,
    y: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
}

fn parse_geom_shell_stdout(text: &str) -> ShellGeom {
    let mut g = ShellGeom::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("X=") {
            g.x = v.trim().parse().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("Y=") {
            g.y = v.trim().parse().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("WIDTH=") {
            g.width = v.trim().parse().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("HEIGHT=") {
            g.height = v.trim().parse().ok();
        }
    }
    g
}

async fn grab_window_geom(wid: u32, display: Option<&str>) -> anyhow::Result<ShellGeom> {
    let wid_s = wid.to_string();
    let out = tokio::time::timeout(XDOTOOL_SUBPROCESS_TIMEOUT, {
        let mut c = Command::new("xdotool");
        linux_xdotool_apply_display(&mut c, display);
        c.args(["getwindowgeometry", "--shell", &wid_s]).output()
    })
    .await??;

    anyhow::ensure!(out.status.success(), "getwindowgeometry failed");
    Ok(parse_geom_shell_stdout(&String::from_utf8_lossy(
        &out.stdout,
    )))
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
    clk.args(["click", "--window", wid_str, "1"]);

    anyhow::ensure!(
        tokio::time::timeout(XDOTOOL_SUBPROCESS_TIMEOUT, clk.status())
            .await??
            .success(),
        "xdotool click --window failed"
    );

    Ok(())
}
