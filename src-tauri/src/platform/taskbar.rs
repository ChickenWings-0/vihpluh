//! Taskbar thumbnail toolbar (Windows): Previous · Play/Pause · Next under the window's
//! taskbar preview. Elsewhere this does nothing — Linux desktops get the same three controls
//! from MPRIS (`platform::media`).
//!
//! Everything here runs on the main thread: the toolbar belongs to the window's thread.

use super::media::MediaCommand;

/// The three buttons, in toolbar order. The discriminant is the button id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Previous = 0,
    Toggle = 1,
    Next = 2,
}

impl Button {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::Previous),
            1 => Some(Self::Toggle),
            2 => Some(Self::Next),
            _ => None,
        }
    }

    pub fn command(self) -> MediaCommand {
        match self {
            Self::Previous => MediaCommand::Previous,
            Self::Toggle => MediaCommand::Toggle,
            Self::Next => MediaCommand::Next,
        }
    }
}

/// The glyph drawn on a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Previous,
    Play,
    Pause,
    Next,
}

fn in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let side = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| {
        (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1)
    };
    let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// Whether the point (`x`, `y`), each in 0–1, is inside the glyph.
fn inside(glyph: Glyph, x: f32, y: f32) -> bool {
    let bar = |from: f32, to: f32| (from..to).contains(&x) && (0.2..0.8).contains(&y);
    match glyph {
        Glyph::Play => in_triangle((x, y), (0.28, 0.16), (0.28, 0.84), (0.86, 0.5)),
        Glyph::Pause => bar(0.24, 0.42) || bar(0.58, 0.76),
        Glyph::Next => {
            in_triangle((x, y), (0.18, 0.2), (0.18, 0.8), (0.66, 0.5)) || bar(0.68, 0.82)
        }
        Glyph::Previous => inside(Glyph::Next, 1.0 - x, y),
    }
}

/// The glyph as coverage per pixel (0–255), `size` × `size`, row by row from the top.
/// Each pixel is sampled 4 × 4 so the diagonals are smooth.
pub fn rasterize(glyph: Glyph, size: usize) -> Vec<u8> {
    const SAMPLES: usize = 4;
    let mut out = Vec::with_capacity(size * size);
    for row in 0..size {
        for column in 0..size {
            let mut hits = 0usize;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (column as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    let y = (row as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    if inside(glyph, x, y) {
                        hits += 1;
                    }
                }
            }
            out.push((hits * 255 / (SAMPLES * SAMPLES)) as u8);
        }
    }
    out
}

/// Adds the toolbar to the window's taskbar button. `on_click` runs on the main thread.
/// Returns false where there is no such toolbar.
pub fn install(window: &tauri::WebviewWindow, on_click: impl Fn(MediaCommand) + 'static) -> bool {
    #[cfg(windows)]
    {
        super::taskbar_win::install(window, Box::new(on_click))
    }
    #[cfg(not(windows))]
    {
        let _ = (window, &on_click);
        false
    }
}

/// Shows Pause while playing and Play otherwise. Main thread only; a no-op before `install`.
pub fn set_playing(playing: bool) {
    #[cfg(windows)]
    super::taskbar_win::set_playing(playing);
    #[cfg(not(windows))]
    let _ = playing;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_ids_map_to_transport_commands() {
        assert_eq!(Button::from_id(0).map(Button::command), Some(MediaCommand::Previous));
        assert_eq!(Button::from_id(1).map(Button::command), Some(MediaCommand::Toggle));
        assert_eq!(Button::from_id(2).map(Button::command), Some(MediaCommand::Next));
        assert_eq!(Button::from_id(3), None);
        assert_eq!(Button::Next as u32, 2);
    }

    #[test]
    fn glyphs_are_drawn_where_expected() {
        let size = 16;
        let at = |glyph, x: usize, y: usize| rasterize(glyph, size)[y * size + x];
        // Play: solid in the left-centre, empty in the corners and at the far left.
        assert_eq!(at(Glyph::Play, 6, 8), 255);
        assert_eq!(at(Glyph::Play, 0, 0), 0);
        assert_eq!(at(Glyph::Play, 15, 1), 0);
        assert_eq!(at(Glyph::Play, 1, 8), 0);
        // Pause: two bars with a gap between them.
        assert_eq!(at(Glyph::Pause, 5, 8), 255);
        assert_eq!(at(Glyph::Pause, 10, 8), 255);
        assert_eq!(at(Glyph::Pause, 8, 8), 0);
        // Next and Previous mirror each other.
        let next = rasterize(Glyph::Next, size);
        let previous = rasterize(Glyph::Previous, size);
        for y in 0..size {
            for x in 0..size {
                assert_eq!(next[y * size + x], previous[y * size + (size - 1 - x)], "{x},{y}");
            }
        }
        assert_eq!(at(Glyph::Next, 12, 8), 255, "the bar");
        // Edges are anti-aliased: some pixels are partly covered.
        assert!(rasterize(Glyph::Play, size).iter().any(|&c| c > 0 && c < 255));
        // Every glyph covers a sensible share of its square.
        for glyph in [Glyph::Previous, Glyph::Play, Glyph::Pause, Glyph::Next] {
            let covered: usize = rasterize(glyph, 20).iter().map(|&c| usize::from(c)).sum();
            let share = covered as f32 / (20.0 * 20.0 * 255.0);
            assert!((0.12..0.4).contains(&share), "{glyph:?}: {share}");
        }
    }
}
