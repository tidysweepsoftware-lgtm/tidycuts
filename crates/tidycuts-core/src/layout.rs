//! Split-screen cells (Split Screen templates): a region of the
//! frame a clip is shown in. The clip's picture is scaled to cover the
//! region, cut off at its edges and framed by a border.

use crate::clip::Color;
use crate::time::Time;
use serde::{Deserialize, Serialize};

/// A region of a split-screen layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cell {
    /// Corners as fractions of the frame (0,0 = top-left), going round the
    /// region. A triangle repeats its last corner.
    pub points: [[f64; 2]; 4],
    /// Width of the lines between cells, in pixels of a 1080-pixel-high
    /// frame; 0 = no border.
    pub border: f64,
    pub border_color: Color,
    /// Rounded corners, in pixels of a 1080-pixel-high frame.
    pub radius: f64,
}

impl Default for Cell {
    fn default() -> Self {
        Cell::rect(0.0, 0.0, 1.0, 1.0)
    }
}

/// The border new layouts get.
pub const DEFAULT_BORDER: f64 = 6.0;

/// A cell's outline and the inside of its border, in project pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellShape {
    pub outer: [[f64; 2]; 4],
    /// The picture's area, before rounding its corners by `radius`.
    pub inner: [[f64; 2]; 4],
    pub radius: f64,
}

const EPS: f64 = 1e-6;

impl Cell {
    /// A rectangle from (x0, y0) to (x1, y1), in frame fractions.
    pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Cell {
        Cell::quad([[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
    }

    pub fn quad(points: [[f64; 2]; 4]) -> Cell {
        Cell {
            points,
            border: DEFAULT_BORDER,
            border_color: [1.0, 1.0, 1.0, 1.0],
            radius: 0.0,
        }
    }

    /// The corners with repeats (triangles) left out.
    fn corners(&self) -> Vec<[f64; 2]> {
        let mut out: Vec<[f64; 2]> = Vec::with_capacity(4);
        for p in self.points {
            let same = |q: &[f64; 2]| (q[0] - p[0]).abs() < EPS && (q[1] - p[1]).abs() < EPS;
            if !out.iter().any(same) {
                out.push(p);
            }
        }
        out
    }

    /// The smallest rectangle around the cell: (x0, y0, x1, y1) in frame
    /// fractions.
    pub fn bounds(&self) -> [f64; 4] {
        let xs = self.points.iter().map(|p| p[0]);
        let ys = self.points.iter().map(|p| p[1]);
        [
            xs.clone().fold(f64::MAX, f64::min),
            ys.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
            ys.fold(f64::MIN, f64::max),
        ]
    }

    /// The cell's bounding box in a `w` x `h` frame: its centre as an offset
    /// from the frame centre, and its size. Clips in a cell are positioned
    /// relative to this centre and sized to cover this box.
    pub fn frame_box(&self, w: f64, h: f64) -> ([f64; 2], [f64; 2]) {
        let [x0, y0, x1, y1] = self.bounds();
        (
            [((x0 + x1) * 0.5 - 0.5) * w, ((y0 + y1) * 0.5 - 0.5) * h],
            [((x1 - x0) * w).max(1.0), ((y1 - y0) * h).max(1.0)],
        )
    }

    /// Size of a `pic_w` x `pic_h` picture scaled to just cover the cell's
    /// box in a `w` x `h` frame.
    pub fn cover(&self, w: f64, h: f64, pic_w: f64, pic_h: f64) -> [f64; 2] {
        let (_, size) = self.frame_box(w, h);
        let (pw, ph) = (pic_w.max(1.0), pic_h.max(1.0));
        let s = (size[0] / pw).max(size[1] / ph);
        [pw * s, ph * s]
    }

    /// The outline and picture area in a `w` x `h` frame. Edges along the
    /// frame's edge get the full border; edges shared with another cell get
    /// half, so the lines between cells are as wide as the outer ones.
    pub fn shape(&self, w: f64, h: f64) -> CellShape {
        let k = w.min(h) / 1080.0;
        let px = |p: [f64; 2]| [p[0] * w, p[1] * h];
        let mut pts: Vec<[f64; 2]> = self.corners().into_iter().map(px).collect();
        let outer = pad4(&pts);
        if pts.len() < 3 {
            return CellShape {
                outer,
                inner: outer,
                radius: 0.0,
            };
        }
        // Clockwise on screen (y down) means a positive signed area.
        let area: f64 = (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        if area < 0.0 {
            pts.reverse();
        }
        let n = pts.len();
        let border = self.border.max(0.0) * k;
        let radius = self.radius.max(0.0) * k;
        let on_edge = |a: [f64; 2], b: [f64; 2]| {
            let near = |v: f64, edge: f64| (v - edge).abs() < 0.5;
            (near(a[0], 0.0) && near(b[0], 0.0))
                || (near(a[0], w) && near(b[0], w))
                || (near(a[1], 0.0) && near(b[1], 0.0))
                || (near(a[1], h) && near(b[1], h))
        };
        // Each edge as a point and an inward normal, moved in by its inset.
        let lines: Vec<([f64; 2], [f64; 2])> = (0..n)
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let len = (dx * dx + dy * dy).sqrt().max(EPS);
                // Inward for a clockwise outline with y pointing down.
                let nrm = [-dy / len, dx / len];
                let d = if on_edge(a, b) { border } else { border * 0.5 } + radius;
                ([a[0] + nrm[0] * d, a[1] + nrm[1] * d], [dx, dy])
            })
            .collect();
        let mut inner: Vec<[f64; 2]> = (0..n)
            .map(|i| {
                let (p1, d1) = lines[(i + n - 1) % n];
                let (p2, d2) = lines[i];
                intersect(p1, d1, p2, d2).unwrap_or(p2)
            })
            .collect();
        // A cell too small for its border keeps a sliver at its centre.
        let c = centroid(&pts);
        let shrunk = (0..n).any(|i| {
            let (p, d) = lines[i];
            let nrm = [-d[1], d[0]];
            (c[0] - p[0]) * nrm[0] + (c[1] - p[1]) * nrm[1] < 0.0
        });
        if shrunk {
            inner = vec![c; n];
        }
        CellShape {
            outer,
            inner: pad4(&inner),
            radius,
        }
    }
}

fn centroid(pts: &[[f64; 2]]) -> [f64; 2] {
    let n = pts.len().max(1) as f64;
    [
        pts.iter().map(|p| p[0]).sum::<f64>() / n,
        pts.iter().map(|p| p[1]).sum::<f64>() / n,
    ]
}

/// Four corners, repeating the last one for a triangle.
fn pad4(pts: &[[f64; 2]]) -> [[f64; 2]; 4] {
    let last = pts.last().copied().unwrap_or([0.0, 0.0]);
    [0, 1, 2, 3].map(|i| pts.get(i).copied().unwrap_or(last))
}

/// Where the line through `p1` along `d1` meets the one through `p2` along `d2`.
fn intersect(p1: [f64; 2], d1: [f64; 2], p2: [f64; 2], d2: [f64; 2]) -> Option<[f64; 2]> {
    let den = d1[0] * d2[1] - d1[1] * d2[0];
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / den;
    Some([p1[0] + d1[0] * t, p1[1] + d1[1] * t])
}

// ----- the ready-made layouts ---------------------------------------------------

/// A ready-made split-screen layout: a name, a group and its cells.
pub struct LayoutDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    /// Each cell as frame fractions (x0, y0, x1, y1) or, for a slanted one,
    /// its four corners.
    cells: &'static [CellDef],
}

#[derive(Clone, Copy)]
enum CellDef {
    Rect(f64, f64, f64, f64),
    Quad([[f64; 2]; 4]),
}

impl LayoutDef {
    pub fn cells(&self) -> Vec<Cell> {
        self.cells
            .iter()
            .map(|c| match *c {
                CellDef::Rect(x0, y0, x1, y1) => Cell::rect(x0, y0, x1, y1),
                CellDef::Quad(p) => Cell::quad(p),
            })
            .collect()
    }
}

use CellDef::{Quad, Rect};

const THIRD: f64 = 1.0 / 3.0;
const TWO_THIRDS: f64 = 2.0 / 3.0;

/// Split-screen layouts, from two cells up to nine.
pub static LAYOUTS: &[LayoutDef] = &[
    LayoutDef {
        id: "two-side",
        name: "Side by Side",
        category: "Two",
        cells: &[Rect(0.0, 0.0, 0.5, 1.0), Rect(0.5, 0.0, 1.0, 1.0)],
    },
    LayoutDef {
        id: "two-stack",
        name: "Top and Bottom",
        category: "Two",
        cells: &[Rect(0.0, 0.0, 1.0, 0.5), Rect(0.0, 0.5, 1.0, 1.0)],
    },
    LayoutDef {
        id: "two-wide-left",
        name: "Wide and Narrow",
        category: "Two",
        cells: &[
            Rect(0.0, 0.0, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, 0.0, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "two-diagonal",
        name: "Diagonal",
        category: "Two",
        cells: &[
            Quad([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 1.0]]),
            Quad([[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 1.0]]),
        ],
    },
    LayoutDef {
        id: "three-columns",
        name: "Three Columns",
        category: "Three",
        cells: &[
            Rect(0.0, 0.0, THIRD, 1.0),
            Rect(THIRD, 0.0, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, 0.0, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "three-rows",
        name: "Three Rows",
        category: "Three",
        cells: &[
            Rect(0.0, 0.0, 1.0, THIRD),
            Rect(0.0, THIRD, 1.0, TWO_THIRDS),
            Rect(0.0, TWO_THIRDS, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "three-left-big",
        name: "Big Left, Two Right",
        category: "Three",
        cells: &[
            Rect(0.0, 0.0, 0.5, 1.0),
            Rect(0.5, 0.0, 1.0, 0.5),
            Rect(0.5, 0.5, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "three-top-big",
        name: "Big Top, Two Below",
        category: "Three",
        cells: &[
            Rect(0.0, 0.0, 1.0, 0.5),
            Rect(0.0, 0.5, 0.5, 1.0),
            Rect(0.5, 0.5, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "four-grid",
        name: "Four Squares",
        category: "Four",
        cells: &[
            Rect(0.0, 0.0, 0.5, 0.5),
            Rect(0.5, 0.0, 1.0, 0.5),
            Rect(0.0, 0.5, 0.5, 1.0),
            Rect(0.5, 0.5, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "four-left-big",
        name: "Big Left, Three Right",
        category: "Four",
        cells: &[
            Rect(0.0, 0.0, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, 0.0, 1.0, THIRD),
            Rect(TWO_THIRDS, THIRD, 1.0, TWO_THIRDS),
            Rect(TWO_THIRDS, TWO_THIRDS, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "four-top-big",
        name: "Big Top, Three Below",
        category: "Four",
        cells: &[
            Rect(0.0, 0.0, 1.0, TWO_THIRDS),
            Rect(0.0, TWO_THIRDS, THIRD, 1.0),
            Rect(THIRD, TWO_THIRDS, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, TWO_THIRDS, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "four-columns",
        name: "Four Columns",
        category: "Four",
        cells: &[
            Rect(0.0, 0.0, 0.25, 1.0),
            Rect(0.25, 0.0, 0.5, 1.0),
            Rect(0.5, 0.0, 0.75, 1.0),
            Rect(0.75, 0.0, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "five-mosaic",
        name: "Mosaic of Five",
        category: "Five and more",
        cells: &[
            Rect(0.0, 0.0, 0.5, 0.5),
            Rect(0.5, 0.0, 1.0, 0.5),
            Rect(0.0, 0.5, THIRD, 1.0),
            Rect(THIRD, 0.5, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, 0.5, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "six-grid",
        name: "Six Squares",
        category: "Five and more",
        cells: &[
            Rect(0.0, 0.0, THIRD, 0.5),
            Rect(THIRD, 0.0, TWO_THIRDS, 0.5),
            Rect(TWO_THIRDS, 0.0, 1.0, 0.5),
            Rect(0.0, 0.5, THIRD, 1.0),
            Rect(THIRD, 0.5, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, 0.5, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "nine-grid",
        name: "Nine Squares",
        category: "Five and more",
        cells: &[
            Rect(0.0, 0.0, THIRD, THIRD),
            Rect(THIRD, 0.0, TWO_THIRDS, THIRD),
            Rect(TWO_THIRDS, 0.0, 1.0, THIRD),
            Rect(0.0, THIRD, THIRD, TWO_THIRDS),
            Rect(THIRD, THIRD, TWO_THIRDS, TWO_THIRDS),
            Rect(TWO_THIRDS, THIRD, 1.0, TWO_THIRDS),
            Rect(0.0, TWO_THIRDS, THIRD, 1.0),
            Rect(THIRD, TWO_THIRDS, TWO_THIRDS, 1.0),
            Rect(TWO_THIRDS, TWO_THIRDS, 1.0, 1.0),
        ],
    },
    LayoutDef {
        id: "frame-in-frame",
        name: "Picture in Picture",
        category: "Five and more",
        cells: &[Rect(0.0, 0.0, 1.0, 1.0), Rect(0.62, 0.62, 0.96, 0.96)],
    },
];

/// The layout with this id.
pub fn layout(id: &str) -> Option<&'static LayoutDef> {
    LAYOUTS.iter().find(|l| l.id == id)
}

impl crate::project::Project {
    /// Lays an empty cell per entry of `cells` on the timeline from `at` for
    /// `len`, each on its own video track above the main one (existing free
    /// tracks first, new ones as needed), named "Cell 1", "Cell 2"… Returns
    /// the placeholders, or nothing (and changes nothing) when there are
    /// not enough video tracks.
    pub fn add_layout(&mut self, cells: &[Cell], at: Time, len: Time) -> Vec<crate::clip::ClipId> {
        let before = self.clone();
        let mut out = Vec::new();
        for (i, cell) in cells.iter().enumerate() {
            let Some(track) = self.overlay_track_for(at, len) else {
                *self = before;
                return Vec::new();
            };
            let Some(id) = self.add_generated(crate::clip::Generator::Placeholder, len, track, at)
            else {
                *self = before;
                return Vec::new();
            };
            if let Some(c) = self.clip_mut(id) {
                c.video.cell = Some(cell.clone());
                c.name = Some(format!("Cell {}", i + 1));
            }
            out.push(id);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn a_left_half_gets_full_outer_and_half_inner_borders() {
        let mut c = Cell::rect(0.0, 0.0, 0.5, 1.0);
        c.border = 10.0;
        let s = c.shape(1920.0, 1080.0);
        assert!(close(s.outer[0], [0.0, 0.0]));
        assert!(close(s.outer[2], [960.0, 1080.0]));
        // Left, top and bottom are frame edges (10 px); the right edge is
        // shared with the other half (5 px).
        assert!(close(s.inner[0], [10.0, 10.0]), "{:?}", s.inner);
        assert!(close(s.inner[2], [955.0, 1070.0]), "{:?}", s.inner);
    }

    #[test]
    fn counter_clockwise_points_and_radius_inset_the_same_way() {
        let mut c = Cell::quad([[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
        c.border = 0.0;
        c.radius = 20.0;
        let s = c.shape(1080.0, 1080.0);
        let xs: Vec<f64> = s.inner.iter().map(|p| p[0]).collect();
        assert!(xs
            .iter()
            .all(|x| (*x - 20.0).abs() < 1e-6 || (*x - 1060.0).abs() < 1e-6));
        assert_eq!(s.radius, 20.0);
    }

    #[test]
    fn triangles_and_diagonals_stay_inside() {
        let c = Cell::quad([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 1.0]]);
        let s = c.shape(1920.0, 1080.0);
        for p in s.inner {
            assert!(p[0] > 0.0 && p[1] > 0.0 && p[0] / 1920.0 + p[1] / 1080.0 < 1.0);
        }
        assert_eq!(s.inner[2], s.inner[3]);
    }

    #[test]
    fn covering_keeps_the_picture_shape() {
        let c = Cell::rect(0.0, 0.0, 0.5, 0.5);
        let size = c.cover(1920.0, 1080.0, 1280.0, 720.0);
        assert!(close(size, [960.0, 540.0]));
        let tall = c.cover(1920.0, 1080.0, 1080.0, 1920.0);
        assert!((tall[0] - 960.0).abs() < 1e-6 && tall[1] > 540.0);
        let (centre, _) = c.frame_box(1920.0, 1080.0);
        assert!(close(centre, [-480.0, -270.0]));
    }

    #[test]
    fn a_huge_border_leaves_a_point() {
        let mut c = Cell::rect(0.4, 0.4, 0.45, 0.45);
        c.border = 400.0;
        let s = c.shape(1920.0, 1080.0);
        assert!(s.inner.iter().all(|p| close(*p, s.inner[0])));
    }

    #[test]
    fn every_layout_fills_the_frame_without_overlap_apart_from_pip() {
        for l in LAYOUTS {
            let cells = l.cells();
            assert!(cells.len() >= 2, "{}", l.id);
            // Rough coverage check on a grid of sample points.
            let n = 40;
            let mut covered = 0;
            for iy in 0..n {
                for ix in 0..n {
                    let (x, y) = ((ix as f64 + 0.37) / n as f64, (iy as f64 + 0.61) / n as f64);
                    let hits = cells.iter().filter(|c| inside(c, x, y)).count();
                    assert!(
                        hits <= if l.id == "frame-in-frame" { 2 } else { 1 },
                        "{} {x} {y}",
                        l.id
                    );
                    covered += (hits > 0) as usize;
                }
            }
            assert_eq!(covered, n * n, "{} leaves a gap", l.id);
        }
        assert!(layout("six-grid").is_some() && layout("nope").is_none());
    }

    fn inside(c: &Cell, x: f64, y: f64) -> bool {
        // Convex polygon test, either winding.
        let pts: Vec<[f64; 2]> = {
            let mut v: Vec<[f64; 2]> = Vec::new();
            for p in c.points {
                if !v.contains(&p) {
                    v.push(p);
                }
            }
            v
        };
        let mut sign = 0.0;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let cross = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
            if cross.abs() < 1e-12 {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
        true
    }

    #[test]
    fn adding_a_layout_makes_one_empty_cell_per_track() {
        use crate::clip::Generator;
        use crate::project::{Project, TrackKind};
        use crate::time::Time;
        let mut p = Project::default();
        let l = layout("four-grid").unwrap();
        let ids = p.add_layout(
            &l.cells(),
            Time::from_secs_f64(2.0),
            Time::from_secs_f64(6.0),
        );
        assert_eq!(ids.len(), 4);
        let tracks: std::collections::HashSet<_> =
            ids.iter().map(|id| p.clip(*id).unwrap().0.id).collect();
        assert_eq!(tracks.len(), 4, "each cell on its own track");
        for (i, id) in ids.iter().enumerate() {
            let (t, c) = p.clip(*id).unwrap();
            assert_eq!(t.kind, TrackKind::Video);
            assert!(matches!(c.generator, Some(Generator::Placeholder)));
            assert_eq!(c.start, Time::from_secs_f64(2.0));
            assert_eq!(c.name.as_deref(), Some(format!("Cell {}", i + 1).as_str()));
            assert!(c.video.cell.is_some());
        }
        // Asking again lands on the free tracks above; the main track stays empty.
        let main = p.main_track().unwrap();
        assert!(p.track(main).unwrap().clips.is_empty());
    }

    #[test]
    fn a_layout_that_does_not_fit_leaves_the_project_alone() {
        use crate::project::Project;
        use crate::time::Time;
        let mut p = Project::default();
        let nine = layout("nine-grid").unwrap().cells();
        // Fill most video tracks first, at the same time.
        assert_eq!(
            p.add_layout(&nine, Time::ZERO, Time::from_secs_f64(1.0))
                .len(),
            9
        );
        let snapshot = p.clone();
        let four = layout("four-grid").unwrap().cells();
        assert!(p
            .add_layout(&four, Time::ZERO, Time::from_secs_f64(1.0))
            .is_empty());
        assert_eq!(p, snapshot);
    }
}
