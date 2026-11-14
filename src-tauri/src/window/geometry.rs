//! Window geometry arithmetic (DESIGN_SYSTEM §9): the picture-in-picture size and corner,
//! aspect-locked resizing, and whether a remembered window still lands on a screen.
//! Everything here is plain numbers in physical pixels, so it is tested without a window.

use serde::{Deserialize, Serialize};

use crate::config::schema::Corner;

/// A rectangle on the virtual desktop, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// The part of `self` that lies inside `other`, as (width, height).
    fn overlap(&self, other: &Rect) -> (i32, i32) {
        let w = self.right().min(other.right()) - self.x.max(other.x);
        let h = self.bottom().min(other.bottom()) - self.y.max(other.y);
        (w.max(0), h.max(0))
    }
}

/// The smallest picture-in-picture window, in logical pixels.
pub const PIP_MIN: (f64, f64) = (240.0, 135.0);
/// Gap between the picture-in-picture window and the work-area edges, logical pixels.
pub const PIP_MARGIN: f64 = 24.0;
/// The shape used when nothing with a picture is loaded.
pub const DEFAULT_ASPECT: f64 = 16.0 / 9.0;

/// Which edge or corner is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ts_rs::TS)]
#[ts(export)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// An aspect ratio that is safe to divide by.
pub fn sane_aspect(aspect: Option<f64>) -> f64 {
    match aspect {
        Some(a) if a.is_finite() && (0.2..=10.0).contains(&a) => a,
        _ => DEFAULT_ASPECT,
    }
}

/// Size for a width and a picture shape, never below the minimum in either direction.
/// `scale` is the monitor's scale factor (the minimum is in logical pixels).
pub fn pip_size(width: f64, aspect: f64, scale: f64) -> (u32, u32) {
    let (min_w, min_h) = (PIP_MIN.0 * scale, PIP_MIN.1 * scale);
    let mut w = width.max(min_w);
    let mut h = w / aspect;
    if h < min_h {
        h = min_h;
        w = h * aspect;
    }
    (w.round() as u32, h.round() as u32)
}

/// Top-left position that puts a window of `size` in `corner` of the work area.
pub fn pip_position(work: Rect, size: (u32, u32), corner: Corner, margin: i32) -> (i32, i32) {
    let left = work.x + margin;
    let top = work.y + margin;
    let right = work.right() - margin - size.0 as i32;
    let bottom = work.bottom() - margin - size.1 as i32;
    // A work area smaller than the window: keep the window's top-left on it.
    let (right, bottom) = (right.max(work.x), bottom.max(work.y));
    match corner {
        Corner::TopLeft => (left, top),
        Corner::TopRight => (right, top),
        Corner::BottomLeft => (left, bottom),
        Corner::BottomRight => (right, bottom),
    }
}

/// The window after dragging `edge` by (`dx`, `dy`) from `start`, keeping `aspect`. The
/// opposite edge (or corner) stays where it was; side edges grow around their centre line.
pub fn pip_resize(
    start: Rect,
    edge: ResizeEdge,
    dx: i32,
    dy: i32,
    aspect: f64,
    scale: f64,
) -> Rect {
    use ResizeEdge::{East, North, NorthEast, NorthWest, South, SouthEast, SouthWest, West};
    let (w0, h0) = (f64::from(start.width), f64::from(start.height));
    // The dragged distance, as growth of the width.
    let grow_w = match edge {
        East | NorthEast | SouthEast => f64::from(dx),
        West | NorthWest | SouthWest => -f64::from(dx),
        North => -f64::from(dy) * aspect,
        South => f64::from(dy) * aspect,
    };
    // Corners follow whichever axis moved further.
    let grow_w = match edge {
        NorthEast | NorthWest => grow_w.max(-f64::from(dy) * aspect),
        SouthEast | SouthWest => grow_w.max(f64::from(dy) * aspect),
        _ => grow_w,
    };
    let (w, h) = pip_size(w0 + grow_w, aspect, scale);
    let (dw, dh) = (w as i32 - w0 as i32, h as i32 - h0 as i32);
    let x = match edge {
        West | NorthWest | SouthWest => start.x - dw,
        North | South => start.x - dw / 2,
        _ => start.x,
    };
    let y = match edge {
        North | NorthEast | NorthWest => start.y - dh,
        East | West => start.y - dh / 2,
        _ => start.y,
    };
    Rect { x, y, width: w, height: h }
}

/// Whether enough of a remembered window is on some monitor to grab it: at least 120 × 60
/// physical pixels of it must be inside one work area.
pub fn is_reachable(window: Rect, monitors: &[Rect]) -> bool {
    monitors.iter().any(|monitor| {
        let (w, h) = window.overlap(monitor);
        w >= 120 && h >= 60
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { x: 0, y: 0, width: 1920, height: 1032 };

    #[test]
    fn size_follows_the_picture_and_respects_the_minimum() {
        assert_eq!(pip_size(480.0, 16.0 / 9.0, 1.0), (480, 270));
        assert_eq!(pip_size(480.0, 4.0 / 3.0, 1.0), (480, 360));
        assert_eq!(pip_size(480.0, 2.39, 1.0), (480, 201));
        // Too narrow: the minimum width wins.
        assert_eq!(pip_size(100.0, 16.0 / 9.0, 1.0), (240, 135));
        // A very wide picture at minimum width would be too short: height wins instead.
        assert_eq!(pip_size(240.0, 4.0, 1.0), (540, 135));
        // A portrait clip keeps its shape.
        assert_eq!(pip_size(240.0, 9.0 / 16.0, 1.0), (240, 427));
        // The minimum scales with the monitor.
        assert_eq!(pip_size(100.0, 16.0 / 9.0, 1.5), (360, 203));
    }

    #[test]
    fn nonsense_aspects_fall_back_to_sixteen_by_nine() {
        assert_eq!(sane_aspect(Some(2.0)), 2.0);
        for bad in [None, Some(0.0), Some(-1.0), Some(f64::NAN), Some(f64::INFINITY), Some(50.0)] {
            assert_eq!(sane_aspect(bad), DEFAULT_ASPECT);
        }
    }

    #[test]
    fn corners_sit_a_margin_inside_the_work_area() {
        let size = (480, 270);
        assert_eq!(pip_position(WORK, size, Corner::BottomRight, 24), (1416, 738));
        assert_eq!(pip_position(WORK, size, Corner::BottomLeft, 24), (24, 738));
        assert_eq!(pip_position(WORK, size, Corner::TopRight, 24), (1416, 24));
        assert_eq!(pip_position(WORK, size, Corner::TopLeft, 24), (24, 24));
        // A second monitor to the left of the primary, with a taskbar at its top.
        let left = Rect { x: -1280, y: 40, width: 1280, height: 984 };
        assert_eq!(pip_position(left, size, Corner::BottomRight, 24), (-504, 730));
        assert_eq!(pip_position(left, size, Corner::TopLeft, 24), (-1256, 64));
        // A work area smaller than the window keeps the window's corner on it.
        let tiny = Rect { x: 0, y: 0, width: 300, height: 200 };
        assert_eq!(pip_position(tiny, size, Corner::BottomRight, 24), (0, 0));
    }

    #[test]
    fn resizing_keeps_the_shape_and_the_opposite_edge() {
        let start = Rect { x: 1000, y: 500, width: 480, height: 270 };
        let a = 16.0 / 9.0;
        let ratio = |r: Rect| f64::from(r.width) / f64::from(r.height);

        let east = pip_resize(start, ResizeEdge::East, 160, 0, a, 1.0);
        assert_eq!((east.width, east.height), (640, 360));
        assert_eq!(east.x, 1000, "left edge stays");
        assert_eq!(east.y, 455, "grows around the centre line");

        let west = pip_resize(start, ResizeEdge::West, -160, 0, a, 1.0);
        assert_eq!((west.width, west.height), (640, 360));
        assert_eq!(west.right(), start.right(), "right edge stays");

        let north_west = pip_resize(start, ResizeEdge::NorthWest, -160, -10, a, 1.0);
        assert_eq!((north_west.right(), north_west.bottom()), (start.right(), start.bottom()));
        assert_eq!(north_west.width, 640);

        let south_east = pip_resize(start, ResizeEdge::SouthEast, 10, 90, a, 1.0);
        assert_eq!((south_east.x, south_east.y), (1000, 500));
        assert_eq!((south_east.width, south_east.height), (640, 360), "the larger axis leads");

        let south = pip_resize(start, ResizeEdge::South, 0, 90, a, 1.0);
        assert_eq!((south.width, south.height), (640, 360));
        assert_eq!(south.y, 500);
        assert_eq!(south.x, 920);

        let north = pip_resize(start, ResizeEdge::North, 0, -90, a, 1.0);
        assert_eq!(north.bottom(), start.bottom());

        for (edge, dx, dy) in [
            (ResizeEdge::East, 333, 7),
            (ResizeEdge::NorthEast, 51, -200),
            (ResizeEdge::SouthWest, -77, 13),
        ] {
            let r = pip_resize(start, edge, dx, dy, a, 1.0);
            assert!((ratio(r) - a).abs() < 0.01, "{edge:?}: {r:?}");
        }
    }

    #[test]
    fn shrinking_stops_at_the_minimum() {
        let start = Rect { x: 1000, y: 500, width: 480, height: 270 };
        let small = pip_resize(start, ResizeEdge::East, -5000, 0, 16.0 / 9.0, 1.0);
        assert_eq!((small.width, small.height), (240, 135));
        assert_eq!(small.x, 1000);
        let small = pip_resize(start, ResizeEdge::NorthWest, 5000, 5000, 16.0 / 9.0, 1.0);
        assert_eq!((small.width, small.height), (240, 135));
        assert_eq!((small.right(), small.bottom()), (start.right(), start.bottom()));
    }

    #[test]
    fn a_remembered_window_must_still_be_on_a_screen() {
        let monitors = [WORK, Rect { x: 1920, y: 0, width: 2560, height: 1400 }];
        let on = |x, y| is_reachable(Rect { x, y, width: 1280, height: 720 }, &monitors);
        assert!(on(100, 100));
        assert!(on(3000, 400), "on the second monitor");
        assert!(on(-1100, 100), "180 px still showing");
        assert!(!on(-1200, 100), "only 80 px showing");
        assert!(!on(100, 1000), "only the top 32 px showing");
        assert!(!on(6000, 100), "the monitor it was on is gone");
        assert!(!is_reachable(Rect { x: 0, y: 0, width: 1280, height: 720 }, &[]));
    }
}
