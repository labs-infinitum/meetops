//! Pale-blue chroma blob + flood fill to locate Chrome’s \"Allow\" pill on the tab-share dialog.
//! Uses the **same pixmap coordinates** as `xdotool mousemove --window` when the PNG was grabbed
//! with `import`/etc. `-window`.
//!
//! Also detects Chrome’s top-right **Restore pages?** bubble via a localized black title band on
//! white card pixels, then returns the close (×) control next to the header.

use image::{GenericImageView, Pixel};

#[derive(Debug, Clone, Copy)]
pub struct TabShareAllowButton {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl TabShareAllowButton {
    pub fn width(&self) -> u32 {
        self.max_x - self.min_x + 1
    }

    pub fn height(&self) -> u32 {
        self.max_y - self.min_y + 1
    }

    pub fn center(&self) -> (u32, u32) {
        ((self.min_x + self.max_x) / 2, (self.min_y + self.max_y) / 2)
    }

    fn area(&self) -> u32 {
        self.width() * self.height()
    }
}

fn is_dialog_pixel(r: u8, g: u8, b: u8) -> bool {
    r > 235 && g > 235 && b > 235
}

fn is_allow_button_pixel(r: u8, g: u8, b: u8) -> bool {
    b > 210 && r >= 180 && g >= 200 && b > r && b >= g && (b as i16 - r as i16) > 15
}

fn bounding_box_from_mask<F>(img: &image::DynamicImage, predicate: F) -> Option<TabShareAllowButton>
where
    F: Fn(u8, u8, u8) -> bool,
{
    let (w, h) = img.dimensions();

    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    let mut found = false;

    for y in 0..h {
        for x in 0..w {
            let pixel = img.get_pixel(x, y).to_rgb();
            let [r, g, b] = pixel.0;

            if predicate(r, g, b) {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if found {
        Some(TabShareAllowButton {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    } else {
        None
    }
}

/// Returns window/screenshot-relative bounding box for the pale-blue Allow control, if found.
pub fn detect_tab_share_allow_button(img: &image::DynamicImage) -> Option<TabShareAllowButton> {
    let dialog = bounding_box_from_mask(img, is_dialog_pixel)?;

    let (w, h) = img.dimensions();

    let mut candidates: Vec<TabShareAllowButton> = Vec::new();

    let search_min_x = dialog.min_x + dialog.width() / 2;
    let search_max_x = dialog.max_x.min(w.saturating_sub(1));
    let search_min_y = dialog.min_y + dialog.height() * 65 / 100;
    let search_max_y = dialog.max_y.min(h.saturating_sub(1));

    let mut visited = vec![false; (w * h) as usize];

    let flood_one = |x: u32, y: u32, visited: &mut [bool]| -> Option<TabShareAllowButton> {
        visited[(y * w + x) as usize] = true;

        let mut stack = vec![(x, y)];

        let mut region = TabShareAllowButton {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        };

        while let Some((cx, cy)) = stack.pop() {
            region.min_x = region.min_x.min(cx);
            region.min_y = region.min_y.min(cy);
            region.max_x = region.max_x.max(cx);
            region.max_y = region.max_y.max(cy);

            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let nx = nx as u32;
                let ny = ny as u32;

                if nx < search_min_x || nx > search_max_x || ny < search_min_y || ny > search_max_y
                {
                    continue;
                }

                let nidx = (ny * w + nx) as usize;
                if visited[nidx] {
                    continue;
                }

                visited[nidx] = true;

                let np = img.get_pixel(nx, ny).to_rgb();
                let [nr, ng, nb] = np.0;

                if is_allow_button_pixel(nr, ng, nb) {
                    stack.push((nx, ny));
                }
            }
        }

        Some(region)
    };

    for y in search_min_y..=search_max_y {
        for x in search_min_x..=search_max_x {
            let idx = (y * w + x) as usize;
            if visited[idx] {
                continue;
            }

            let pixel = img.get_pixel(x, y).to_rgb();
            let [r, g, b] = pixel.0;

            if !is_allow_button_pixel(r, g, b) {
                visited[idx] = true;
                continue;
            }

            if let Some(region) = flood_one(x, y, &mut visited) {
                if region.width() >= 35
                    && region.width() <= 140
                    && region.height() >= 20
                    && region.height() <= 70
                    && region.area() >= 700
                {
                    candidates.push(region);
                }
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|r| r.center().0);
    candidates.into_iter().last()
}

/// Chrome “Restore pages?” bubble in the top-right (white card + black title band).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestorePagesPopup {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub close_x: u32,
    pub close_y: u32,
}

impl RestorePagesPopup {
    /// Window-relative click target for the × control beside “Restore pages?”.
    pub fn close_button_center(&self) -> (u32, u32) {
        (self.close_x, self.close_y)
    }
}

fn is_restore_title_black(r: u8, g: u8, b: u8) -> bool {
    r < 50 && g < 50 && b < 50
}

fn is_restore_card_white(r: u8, g: u8, b: u8) -> bool {
    r > 230 && g > 230 && b > 230
}

fn is_restore_close_gray(r: u8, g: u8, b: u8) -> bool {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    (80..=230).contains(&max) && max - min <= 25
}

fn count_white_in_row(img: &image::DynamicImage, y: u32, x_start: u32, x_end: u32) -> u32 {
    let w = img.width();
    if y >= img.height() {
        return 0;
    }
    let x_end = x_end.min(w.saturating_sub(1));
    (x_start..=x_end)
        .filter(|&x| {
            let [r, g, b] = img.get_pixel(x, y).to_rgb().0;
            is_restore_card_white(r, g, b)
        })
        .count() as u32
}

/// Detect the top-right **Restore pages?** bubble from a Chrome window screenshot.
pub fn detect_restore_pages_popup(img: &image::DynamicImage) -> Option<RestorePagesPopup> {
    let (w, h) = img.dimensions();
    if w < 400 || h < 200 {
        return None;
    }

    let search_min_x = w * 55 / 100;
    let search_max_y = h * 35 / 100;

    let mut card: Option<(u32, u32, u32)> = None;
    for y in 70..search_max_y.min(h) {
        let mut x = search_min_x;
        while x < w {
            while x < w {
                let [r, g, b] = img.get_pixel(x, y).to_rgb().0;
                if is_restore_card_white(r, g, b) {
                    break;
                }
                x += 1;
            }
            if x >= w {
                break;
            }

            let start = x;
            while x < w {
                let [r, g, b] = img.get_pixel(x, y).to_rgb().0;
                if !is_restore_card_white(r, g, b) {
                    break;
                }
                x += 1;
            }
            let end = x.saturating_sub(1);
            let width = end - start + 1;

            if (250..=450).contains(&width) && end >= w.saturating_sub(40) && start >= w * 70 / 100
            {
                card = Some((start, end, y));
                break;
            }
        }
        if card.is_some() {
            break;
        }
    }

    let (card_left, card_right, card_top) = card?;
    let card_width = card_right - card_left + 1;

    let title_dark = (card_top + 12..=(card_top + 45).min(h.saturating_sub(1)))
        .flat_map(|y| (card_left + 8..=(card_left + 190).min(card_right)).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let [r, g, b] = img.get_pixel(x, y).to_rgb().0;
            is_restore_title_black(r, g, b)
        })
        .count();
    if title_dark < 20 {
        return None;
    }

    let mut close_pts: Vec<(u32, u32)> = Vec::new();
    let close_min_x = card_right.saturating_sub(45);
    let close_max_x = card_right.saturating_sub(10);
    let close_min_y = card_top + 12;
    let close_max_y = (card_top + 45).min(h.saturating_sub(1));
    for y in close_min_y..=close_max_y {
        for x in close_min_x..=close_max_x {
            let [r, g, b] = img.get_pixel(x, y).to_rgb().0;
            if is_restore_close_gray(r, g, b) {
                close_pts.push((x, y));
            }
        }
    }
    if close_pts.len() < 4 {
        return None;
    }

    let close_min_x = close_pts.iter().map(|p| p.0).min()?;
    let close_max_x = close_pts.iter().map(|p| p.0).max()?;
    let close_min_y = close_pts.iter().map(|p| p.1).min()?;
    let close_max_y = close_pts.iter().map(|p| p.1).max()?;
    if close_max_x - close_min_x > 20 || close_max_y - close_min_y > 20 {
        return None;
    }

    let mut card_bottom = card_top;
    for y in (card_top + 1)..search_max_y.min(h) {
        if count_white_in_row(img, y, card_left, card_right) > card_width * 45 / 100 {
            card_bottom = y;
        } else if card_bottom.saturating_sub(card_top) >= 90 {
            break;
        }
    }
    if card_bottom.saturating_sub(card_top) < 80 {
        return None;
    }

    Some(RestorePagesPopup {
        min_x: card_left,
        min_y: card_top,
        max_x: card_right,
        max_y: card_bottom,
        close_x: (close_min_x + close_max_x) / 2,
        close_y: (close_min_y + close_max_y) / 2,
    })
}

/// Draw a filled dot plus crosshair at `(cx, cy)` for visual regression checks.
pub fn draw_click_marker(img: &mut image::RgbImage, cx: u32, cy: u32, color: [u8; 3], radius: i32) {
    use image::Rgb;
    let w = img.width() as i32;
    let h = img.height() as i32;
    let cx_i = cx as i32;
    let cy_i = cy as i32;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let x = cx_i + dx;
            let y = cy_i + dy;
            if (0..w).contains(&x) && (0..h).contains(&y) {
                img.put_pixel(x as u32, y as u32, Rgb(color));
            }
        }
    }

    for d in -radius * 2..=radius * 2 {
        for (x, y) in [(cx_i + d, cy_i), (cx_i, cy_i + d)] {
            if (0..w).contains(&x) && (0..h).contains(&y) {
                img.put_pixel(x as u32, y as u32, Rgb(color));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
