//! Annotate captured Chrome window PNGs with click markers (red = Restore ×, green = Allow).
//!
//! ```text
//! cargo run --bin preview-tab-capture-clicks -- \
//!   ../../data/recordings/staging/.../chrome-tab-share-match-*.png
//! ```
//!
//! Output: `apps/backend/target/detector-preview/<tag>-<stem>-click-preview.png`

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[path = "../bot/allow_button_detect/mod.rs"]
mod allow_button_detect;

const RESTORE_DOT: [u8; 3] = [255, 40, 40];
const ALLOW_DOT: [u8; 3] = [40, 220, 40];

fn usage() -> ! {
    eprintln!(
        "Usage:\n  preview-tab-capture-clicks <screenshot.png> [more.png ...]\n\n\
         Writes annotated PNGs to target/detector-preview/:\n\
           red dot   = Restore pages? close (×)\n\
           green dot = tab-share Allow pill\n",
    );
    std::process::exit(1);
}

fn preview_out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/detector-preview")
}

fn annotate_and_write(path: &Path, out_dir: &Path) -> Result<()> {
    let img = image::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut out = img.to_rgb8();

    let restore = allow_button_detect::detect_restore_pages_popup(&img);
    let allow = allow_button_detect::detect_tab_share_allow_button(&img);

    if let Some(popup) = restore {
        let (cx, cy) = popup.close_button_center();
        allow_button_detect::restore_popup::draw_click_marker(&mut out, cx, cy, RESTORE_DOT, 10);
        println!(
            "{}: RESTORE × at ({cx}, {cy})  card {}..{},{}..{}",
            path.display(),
            popup.min_x,
            popup.max_x,
            popup.min_y,
            popup.max_y,
        );
    }

    if let Some(button) = allow {
        let (cx, cy) = button.center();
        allow_button_detect::restore_popup::draw_click_marker(&mut out, cx, cy, ALLOW_DOT, 10);
        println!(
            "{}: ALLOW at ({cx}, {cy})  pill {}..{},{}..{}",
            path.display(),
            button.min_x,
            button.max_x,
            button.min_y,
            button.max_y,
        );
    }

    if restore.is_none() && allow.is_none() {
        println!("{}: no Restore or Allow target detected", path.display());
    }

    let tag = match (restore.is_some(), allow.is_some()) {
        (true, false) => "restore",
        (false, true) => "allow",
        (true, true) => "both",
        (false, false) => "none",
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("shot");
    let out_path = out_dir.join(format!("{tag}-{stem}-click-preview.png"));
    out.save(&out_path)
        .with_context(|| format!("write {}", out_path.display()))?;
    println!("  -> {}", out_path.display());
    Ok(())
}

fn main() -> Result<()> {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        usage();
    }

    let out_dir = preview_out_dir();
    std::fs::create_dir_all(&out_dir).context("create target/detector-preview")?;

    for path in paths {
        if !path.is_file() {
            anyhow::bail!("not a file: {}", path.display());
        }
        annotate_and_write(&path, &out_dir)?;
    }

    Ok(())
}
