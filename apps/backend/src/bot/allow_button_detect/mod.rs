//! Screenshot-based detectors for Chrome tab-share UI (Allow pill, Restore pages bubble).

mod restore_popup;
mod tab_share;

#[cfg(test)]
mod tests;

pub use restore_popup::detect_restore_pages_popup;
pub use tab_share::detect_tab_share_allow_button;
#[allow(unused_imports)]
pub use restore_popup::draw_click_marker;
