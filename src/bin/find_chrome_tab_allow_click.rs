//! CLI: locate Chrome tab-share pale-blue Allow pill in a PNG; optional `xdotool getwindowgeometry`
//! helper for noisy absolute coordinates. Detector lives in [`bot::allow_button_detect`] (`#[path]`).
//!
//! For automation prefer `cargo run`/binary from this crate and pass window id to correlate — or rely on
//! the record command (`tab_capture_allow`) which clicks via `xdotool --window`.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

#[path = "../bot/allow_button_detect.rs"]
mod allow_button_detect;

#[path = "../bot/xdotool_geom.rs"]
mod xdotool_geom;

fn apply_display(cmd: &mut Command, display: Option<&str>) {
    if let Some(d) = display {
        cmd.env("DISPLAY", d);
    }
}

fn xdot_window_geometry_shell(wid: u32, display: Option<&str>) -> Result<xdotool_geom::ShellGeom> {
    let wid_s = wid.to_string();
    let mut cmd = Command::new("xdotool");
    cmd.args(["getwindowgeometry", "--shell", &wid_s]);
    apply_display(&mut cmd, display);
    let out = cmd
        .output()
        .context("spawn xdotool getwindowgeometry — is xdotool installed?")?;

    anyhow::ensure!(
        out.status.success(),
        "xdotool getwindowgeometry exited {:?}",
        out.status.code()
    );

    Ok(xdotool_geom::parse_geom_shell_stdout(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

fn usage() -> ! {
    eprintln!(
        "Usage:\n  find-chrome-tab-allow-click <screenshot.png> [--window-id <id>] [--display <DISPLAY>]\n\
             cargo run --bin find-chrome-tab-allow-click -- <screenshot.png> [OPTS]\n\n\
             Prints PNG-relative bounds and estimated desktop coords when --window-id is set.\n",
    );
    std::process::exit(1);
}

fn parse_u32_maybe_hex(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

#[derive(Default)]
struct Args {
    image: Option<PathBuf>,
    window_id: Option<u32>,
    display: Option<String>,
}

fn parse_args(mut it: impl Iterator<Item = OsString>) -> Args {
    let _exe = it.next();
    let mut out = Args::default();
    let mut expect_window = false;
    let mut expect_display = false;
    for raw in it {
        let s_lossy = raw.to_string_lossy();
        let s: &str = s_lossy.as_ref();
        if expect_window {
            out.window_id = parse_u32_maybe_hex(s);
            expect_window = false;
            continue;
        }
        if expect_display {
            out.display = Some(s_lossy.into_owned());
            expect_display = false;
            continue;
        }
        match s {
            "-h" | "--help" => usage(),
            "--window-id" | "-w" => expect_window = true,
            "--display" | "-d" => expect_display = true,
            _ if s.starts_with('-') => {
                eprintln!("Unknown flag: {s}");
                usage();
            }
            _ => {
                if out.image.is_some() {
                    eprintln!("Unexpected extra argument: {s}");
                    usage();
                }
                out.image = Some(PathBuf::from(raw));
            }
        }
    }

    if expect_window || expect_display {
        eprintln!("Missing value after flag");
        usage();
    }
    out
}

fn main() -> Result<()> {
    let args = parse_args(std::env::args_os());
    let image_path = args.image.as_ref().context("screenshot path required")?;

    let img = image::open(image_path).with_context(|| format!("open {}", image_path.display()))?;

    let dw = img.width();
    let dh = img.height();

    match allow_button_detect::detect_tab_share_allow_button(&img) {
        Some(button) => {
            let (rx, ry) = button.center();
            println!("Detected Allow button bounds (pixels from PNG top-left):");
            println!("  x: {}..{}", button.min_x, button.max_x);
            println!("  y: {}..{}", button.min_y, button.max_y);
            println!();
            println!("PNG-relative click center:");
            println!("  x = {rx}");
            println!("  y = {ry}");
            println!();
            println!(
                "xdotool (window-relative — same origin as pixmap from `import|magick|maim -window <id>`):"
            );
            println!("  xdotool mousemove --window WINDOW_ID {rx} {ry} click --window WINDOW_ID 1",);

            if let Some(wid) = args.window_id {
                let geo = xdot_window_geometry_shell(wid, args.display.as_deref())?;
                let gx = geo.x.unwrap_or(0);
                let gy = geo.y.unwrap_or(0);

                println!();
                println!("xdotool getwindowgeometry --shell {wid}:");
                println!(
                    "  X={:?} Y={:?} WIDTH={:?} HEIGHT={:?}",
                    geo.x, geo.y, geo.width, geo.height
                );

                match (geo.width, geo.height) {
                    (Some(pw), Some(ph))
                        if pw > 0 && ph > 0 && (pw as u32 != dw || ph as u32 != dh) =>
                    {
                        println!(
                            "  WARNING: PNG is {dw}×{dh} but window geometry is {pw}×{ph}; desktop offset below may not match decorations / HiDPI."
                        );
                    }
                    (Some(pw), Some(ph)) if pw > 0 && ph > 0 => {
                        println!(
                            "  PNG size matches WIDTH×HEIGHT ({dw}×{dh}); desktop offset usually closer."
                        );
                    }
                    _ => println!("  (Could not compare sizes — WIDTH/HEIGHT missing.)"),
                }

                let ax = gx + rx as i32;
                let ay = gy + ry as i32;
                println!();
                println!("Estimated root (desktop) click = window POSITION + PNG-relative center:");
                println!("  x ≈ {ax}");
                println!("  y ≈ {ay}");
                println!();
                println!("xdotool (absolute — only if `--window` fails or you validated offset):");
                println!("  xdotool mousemove {ax} {ay} click 1");
            }
        }
        None => {
            eprintln!("Could not detect Allow button.");
            std::process::exit(2);
        }
    }

    Ok(())
}
