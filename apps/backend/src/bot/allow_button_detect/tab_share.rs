//! Pale-blue chroma blob + flood fill to locate Chrome’s "Allow" pill on the tab-share dialog.

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
