//! Chrome “Restore pages?” bubble in the top-right (white card + black title band).

use image::{GenericImageView, Pixel};

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

/// Used by `preview-tab-capture-clicks` and detector tests (not the main server binary).
#[allow(dead_code)]
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
