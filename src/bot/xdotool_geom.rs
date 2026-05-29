//! Parsed output of `xdotool getwindowgeometry --shell`.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShellGeom {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

pub fn parse_geom_shell_stdout(text: &str) -> ShellGeom {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xdotool_shell_geometry() {
        let g = parse_geom_shell_stdout(
            "WINDOW=12345\nX=10\nY=20\nWIDTH=800\nHEIGHT=600\nSCREEN=0\n",
        );
        assert_eq!(g.x, Some(10));
        assert_eq!(g.y, Some(20));
        assert_eq!(g.width, Some(800));
        assert_eq!(g.height, Some(600));
    }
}
