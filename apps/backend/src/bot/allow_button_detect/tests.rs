use super::{
    detect_restore_pages_popup, detect_tab_share_allow_button,
    restore_popup::draw_click_marker,
};
use image::DynamicImage;
use std::path::{Path, PathBuf};

const RESTORE_DOT: [u8; 3] = [255, 40, 40];
const ALLOW_DOT: [u8; 3] = [40, 220, 40];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn preview_out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/detector-preview")
}

fn annotate_detections(img: &DynamicImage) -> image::RgbImage {
    let mut out = img.to_rgb8();
    if let Some(popup) = detect_restore_pages_popup(img) {
        let (cx, cy) = popup.close_button_center();
        draw_click_marker(&mut out, cx, cy, RESTORE_DOT, 10);
    }
    if let Some(allow) = detect_tab_share_allow_button(img) {
        let (cx, cy) = allow.center();
        draw_click_marker(&mut out, cx, cy, ALLOW_DOT, 10);
    }
    out
}

fn run_fixture_preview(
    path: &Path,
    expect_restore: bool,
    expect_allow: bool,
) -> Option<(bool, bool)> {
    if !path.is_file() {
        return None;
    }
    let img = image::open(path).expect("open fixture png");
    let got_restore = detect_restore_pages_popup(&img).is_some();
    let got_allow = detect_tab_share_allow_button(&img).is_some();
    assert_eq!(
        got_restore,
        expect_restore,
        "restore detection mismatch for {}",
        path.display()
    );
    assert_eq!(
        got_allow,
        expect_allow,
        "allow detection mismatch for {}",
        path.display()
    );
    let out_dir = preview_out_dir();
    std::fs::create_dir_all(&out_dir).expect("create preview dir");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("shot");
    let out_path = out_dir.join(format!("{stem}-click-preview.png"));
    annotate_detections(&img)
        .save(&out_path)
        .unwrap_or_else(|e| panic!("save {}: {e}", out_path.display()));
    Some((got_restore, got_allow))
}

#[test]
fn restore_pages_popup_on_staging_capture() {
    let root = workspace_root();
    let Some(_) = run_fixture_preview(
        &root.join(
            "data/recordings/staging/5d097844-91f6-4592-a995-6331df873eff/5/tab-capture-screenshots/chrome-tab-share-match-1780191850554625508.png",
        ),
        true,
        false,
    ) else {
        return;
    };
}

#[test]
fn tab_share_allow_on_manual_capture() {
    let root = workspace_root();
    let Some(_) = run_fixture_preview(
        &root.join(
            "data/recordings/manual-20260529T105736Z/tab-capture-screenshots/chrome-tab-share-match-1780052293290177846.png",
        ),
        false,
        true,
    ) else {
        return;
    };
}

fn collect_tab_capture_pngs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_tab_capture_pngs(&path, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("chrome-tab-share-match-") && name.ends_with(".png") {
            out.push(path);
        }
    }
}

#[test]
fn preview_all_local_tab_capture_screenshots() {
    let root = workspace_root();
    let data = root.join("data/recordings");
    if !data.is_dir() {
        return;
    }
    let out_dir = preview_out_dir();
    std::fs::create_dir_all(&out_dir).ok();

    let mut paths = Vec::new();
    collect_tab_capture_pngs(&data, &mut paths);

    let mut count = 0_u32;
    for path in paths {
        let img = match image::open(&path) {
            Ok(i) => i,
            Err(_) => continue,
        };
        let got_restore = detect_restore_pages_popup(&img).is_some();
        let got_allow = detect_tab_share_allow_button(&img).is_some();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("shot");
        let tag = match (got_restore, got_allow) {
            (true, false) => "restore",
            (false, true) => "allow",
            (true, true) => "both",
            (false, false) => "none",
        };
        let out_path = out_dir.join(format!("{tag}-{stem}-click-preview.png"));
        annotate_detections(&img)
            .save(&out_path)
            .unwrap_or_else(|e| panic!("save {}: {e}", out_path.display()));
        count += 1;
    }
    assert!(
        count > 0,
        "expected at least one chrome-tab-share-match PNG under data/recordings"
    );
}
