//! CABIN-WEB · BROWSER SHELL
//!   · App: the wasm object the page drives, one per canvas
//!   · Panel: the yard turned to its long axis, track spacing stretched by
//!     PANEL_STRETCH about a measured centreline
//!   · Camera: pan 1:1, rubber band past the yard's edges, zoom on a spring
//!     about the cursor
//!   · skeleton train (B2 only): one block run along a fixed path, so the
//!     draw loop and the hooks have something moving; B4's trains replace it
//!   · draw: tracks, platform and portal labels, the train
//!   · test hooks: read-only, for Playwright
//!
//! Thin on purpose: real frame time and input come in from the page's
//! JavaScript, draw calls go out to a Canvas 2D context. No game rule lives
//! here; if it would matter in a headless test, it belongs in `cabin-sim`.

use cabin_sim::consts::PANEL_STRETCH;
use cabin_sim::yard::{Kind, Yard};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Zoom limits in CSS px per panel metre. 0.1 shows the 2.3 km yard in
/// 230 px; 10 puts two neighbouring platform tracks 400 px apart.
const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 10.0;
/// Log-zoom per wheel pixel. One mouse notch (100 px) zooms by e^0.2 = 1.22x.
const ZOOM_PER_WHEEL_PX: f64 = 0.002;
/// Zoom spring, on log scale: MOTION.gesture in md/designs.md (k 380,
/// zeta 0.75, 369 ms, 2.8% overshoot). A new wheel notch retargets it.
const ZOOM_K: f64 = 380.0;
const ZOOM_C: f64 = 2.0 * 0.75 * 19.493_588; // 2 * zeta * sqrt(k)
/// Pan past an edge: MOTION.rubberBand 0.55 while dragging, and the
/// MOTION.edge spring (k 300, zeta 1: never bounces past) on release.
const RUBBER: f64 = 0.55;
const EDGE_K: f64 = 300.0;
const EDGE_C: f64 = 2.0 * 17.320_508; // 2 * sqrt(k): critical
/// The default view frames the running lines at this share of the canvas.
const FIT_SHARE: f64 = 0.92;

/// Panel centreline: running track is sampled every SAMPLE_M, binned every
/// BIN_M along the long axis, and the median across-track offset of each bin
/// is smoothed over SMOOTH_BINS either side. Drawn shapes are cut into
/// pieces no longer than DENSIFY_M before mapping, so a long straight track
/// bends with the centreline and never crosses its neighbour on screen.
const SAMPLE_M: f64 = 10.0;
const BIN_M: f64 = 50.0;
const SMOOTH_BINS: usize = 2;
const DENSIFY_M: f64 = 25.0;

/// The skeleton train: portal N1 to platform 1 over running track, found by
/// a legal search over the B0 yard (points stem to leg only). Edge indices
/// of `data/ndls.yard` as extracted 2026-11-15; a re-extract that breaks the
/// chain stops the page with a banner, and B4 deletes all of this.
const SKELETON_PATH: [usize; 5] = [80, 79, 78, 41, 40];
/// 50 km/h, the plan's starting SPEED_NORMAL_KMH, in real time.
const SKELETON_SPEED_MS: f64 = 13.9;
const SKELETON_LEN_M: f64 = 400.0;

/// The yard drawn as a control panel: `u` along the station's long axis,
/// `v` across it. Distances along the tracks are true; the spacing between
/// tracks is PANEL_STRETCH times true, measured from a centreline that
/// follows the line's curve. Stretching about a straight axis would stretch
/// the curve as well: the south lines bend 425 m off the axis, and a
/// straight 6x stretch made the panel 3.3 km tall for a 2.3 km yard.
pub struct Panel {
    ox: f64,
    oy: f64,
    cos: f64,
    sin: f64,
    /// `u` of the first bin's centre, and the smoothed centreline per bin.
    u0: f64,
    centre: Vec<f64>,
}

impl Panel {
    pub fn new(yard: &Yard) -> Panel {
        // The long axis is the direction the platform tracks run: the
        // length-weighted mean of their segments' directions, on doubled
        // angles so a segment drawn either way counts the same. Not the
        // principal axis of their points: P1-P6 sit about 400 m along from
        // P7-P16, and that stagger tilted the principal axis 5 degrees off
        // the tracks (-76.6 against -81.5), which the stretch turned into
        // platforms drawn as steep diagonals.
        let plat: Vec<&[(f64, f64)]> = yard.platforms.iter().map(|p| yard.edges[p.edge].poly.as_slice()).collect();
        let n = plat.iter().map(|s| s.len()).sum::<usize>().max(1) as f64;
        let ox = plat.iter().flat_map(|s| s.iter()).map(|p| p.0).sum::<f64>() / n;
        let oy = plat.iter().flat_map(|s| s.iter()).map(|p| p.1).sum::<f64>() / n;
        let (mut c2, mut s2) = (0.0, 0.0);
        for w in plat.iter().flat_map(|s| s.windows(2)) {
            let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let (len, phi) = (dx.hypot(dy), dy.atan2(dx));
            c2 += len * (2.0 * phi).cos();
            s2 += len * (2.0 * phi).sin();
        }
        let th = 0.5 * s2.atan2(c2);
        let mut p = Panel { ox, oy, cos: th.cos(), sin: th.sin(), u0: 0.0, centre: Vec::new() };

        let mut samples: Vec<(f64, f64)> = Vec::new();
        for e in yard.edges.iter().filter(|e| e.kind == Kind::Running) {
            samples.extend(densify(&e.poly, SAMPLE_M).into_iter().map(|(x, y)| p.rotate(x, y)));
        }
        if samples.is_empty() {
            return p;
        }
        let umin = samples.iter().map(|s| s.0).fold(f64::MAX, f64::min);
        let umax = samples.iter().map(|s| s.0).fold(f64::MIN, f64::max);
        let bins = ((umax - umin) / BIN_M).floor() as usize + 1;
        let mut by_bin: Vec<Vec<f64>> = vec![Vec::new(); bins];
        for &(u, v) in &samples {
            by_bin[(((u - umin) / BIN_M) as usize).min(bins - 1)].push(v);
        }
        let mut med: Vec<Option<f64>> = by_bin
            .into_iter()
            .map(|mut vs| {
                vs.sort_by(f64::total_cmp);
                (!vs.is_empty()).then(|| vs[vs.len() / 2])
            })
            .collect();
        // A bin with no running track takes its nearest filled neighbour.
        for i in 0..bins {
            if med[i].is_none() {
                let near = (1..bins).find_map(|d| {
                    let lo = i.checked_sub(d).and_then(|j| med[j]);
                    lo.or_else(|| med.get(i + d).copied().flatten())
                });
                med[i] = near;
            }
        }
        let med: Vec<f64> = med.into_iter().map(|m| m.unwrap_or(0.0)).collect();
        p.centre = (0..bins)
            .map(|i| {
                let (a, b) = (i.saturating_sub(SMOOTH_BINS), (i + SMOOTH_BINS).min(bins - 1));
                med[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
            })
            .collect();
        p.u0 = umin + BIN_M / 2.0;
        p
    }

    /// Turn to the long axis, true scale. A rotation, never a mirror: the
    /// panel keeps the yard's handedness.
    fn rotate(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = (x - self.ox, y - self.oy);
        (dx * self.cos + dy * self.sin, -dx * self.sin + dy * self.cos)
    }

    fn centre_v(&self, u: f64) -> f64 {
        let c = &self.centre;
        if c.is_empty() {
            return 0.0;
        }
        let t = ((u - self.u0) / BIN_M).clamp(0.0, (c.len() - 1) as f64);
        let i = (t.floor() as usize).min(c.len() - 1);
        let j = (i + 1).min(c.len() - 1);
        c[i] + (c[j] - c[i]) * (t - i as f64)
    }

    /// World metres (east, north) to panel metres. For a fixed `u` the map
    /// is increasing in `v`, so tracks keep their order and never cross on
    /// screen where they do not cross on the ground.
    pub fn map(&self, x: f64, y: f64) -> (f64, f64) {
        let (u, v) = self.rotate(x, y);
        let c = self.centre_v(u);
        (u, c + PANEL_STRETCH * (v - c))
    }

    pub fn map_poly(&self, poly: &[(f64, f64)]) -> Vec<(f64, f64)> {
        densify(poly, DENSIFY_M).into_iter().map(|(x, y)| self.map(x, y)).collect()
    }
}

/// The same polyline with extra vertices, no piece longer than `step`.
fn densify(poly: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(poly.len());
    for (k, &b) in poly.iter().enumerate() {
        if let Some(&a) = k.checked_sub(1).and_then(|j| poly.get(j)) {
            let pieces = ((b.0 - a.0).hypot(b.1 - a.1) / step).ceil().max(1.0) as usize;
            for i in 1..pieces {
                let t = i as f64 / pieces as f64;
                out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
        }
        out.push(b);
    }
    out
}

/// A run of edges joined end to end into one oriented polyline in world
/// metres, with the distance along it at each vertex.
struct Line {
    pts: Vec<(f64, f64)>,
    at: Vec<f64>,
}

impl Line {
    fn along(yard: &Yard, path: &[usize]) -> Result<Line, String> {
        let edge = |i: usize| yard.edges.get(i).ok_or_else(|| format!("skeleton path: no edge {i}"));
        let first = edge(path[0])?;
        // The first edge runs away from the node it shares with the second.
        let mut at_node = match path.get(1).map(|&i| edge(i)).transpose()? {
            Some(next) if first.b == next.a || first.b == next.b => first.a,
            Some(next) if first.a == next.a || first.a == next.b => first.b,
            Some(_) => return Err(format!("skeleton path: edges {} and {} do not meet", path[0], path[1])),
            None => first.a,
        };
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for &i in path {
            let e = edge(i)?;
            let mut poly = e.poly.clone();
            if e.a == at_node {
                at_node = e.b;
            } else if e.b == at_node {
                at_node = e.a;
                poly.reverse();
            } else {
                return Err(format!("skeleton path: edge {i} does not start where the last one ended"));
            }
            let skip = usize::from(!pts.is_empty());
            pts.extend(poly.into_iter().skip(skip));
        }
        let mut at = vec![0.0];
        for w in pts.windows(2) {
            at.push(at[at.len() - 1] + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
        }
        Ok(Line { pts, at })
    }

    fn len(&self) -> f64 {
        self.at[self.at.len() - 1]
    }

    fn point(&self, s: f64) -> (f64, f64) {
        let s = s.clamp(0.0, self.len());
        let k = self.at.partition_point(|&a| a < s).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[k - 1], self.pts[k]);
        let span = self.at[k] - self.at[k - 1];
        let t = if span > 0.0 { (s - self.at[k - 1]) / span } else { 0.0 };
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }

    /// The stretch of line from `s0` to `s1`, with its inner vertices.
    fn piece(&self, s0: f64, s1: f64) -> Vec<(f64, f64)> {
        let mut out = vec![self.point(s0)];
        out.extend(self.pts.iter().zip(&self.at).filter(|&(_, &a)| a > s0 && a < s1).map(|(p, _)| *p));
        out.push(self.point(s1));
        out
    }
}

struct Camera {
    /// Panel point at the canvas centre, metres.
    cx: f64,
    cy: f64,
    /// Edge-spring velocity of the centre, metres per second.
    vx: f64,
    vy: f64,
    /// ln(CSS px per panel metre), its spring target and velocity.
    z: f64,
    zt: f64,
    vz: f64,
    /// Zoom pivot: a screen point and the panel point that must stay under it.
    pivot: Option<(f64, f64, f64, f64)>,
    /// Where a drag would have put the centre with no edges; the drawn
    /// centre is this, rubber-banded. None when no drag is under way.
    raw: Option<(f64, f64)>,
    fitted: bool,
}

impl Camera {
    fn scale(&self) -> f64 {
        self.z.exp()
    }
}

/// The rubber band of md/designs.md: `d * (1 - 1 / (x * c / d + 1))` for an
/// overshoot `x` past an edge, where `d` is the view's size. Same units in
/// and out; it never reaches `d`.
fn rubber(x: f64, d: f64) -> f64 {
    d * (1.0 - 1.0 / (x * RUBBER / d + 1.0))
}

/// One axis of a dragged centre: free inside [lo, hi], banded outside.
/// `x`, `lo`, `hi` in panel metres; `d_m` is the view's size in metres.
fn band(x: f64, lo: f64, hi: f64, d_m: f64) -> f64 {
    if x < lo {
        lo - rubber(lo - x, d_m)
    } else if x > hi {
        hi + rubber(x - hi, d_m)
    } else {
        x
    }
}

/// Route Rust panics to the console with their message. Without this a
/// panic surfaces as an opaque "unreachable executed" and the cause is lost.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("cabin panic: {info}")));
    }));
}

#[wasm_bindgen]
pub struct App {
    yard: Yard,
    panel: Panel,
    /// Each edge's shape in panel metres, mapped once at load.
    shapes: Vec<Vec<(f64, f64)>>,
    /// Panel bounds of everything drawn, and of the running lines alone.
    all: (f64, f64, f64, f64),
    running: (f64, f64, f64, f64),
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    cam: Camera,
    w: f64,
    h: f64,
    dpr: f64,
    line: Line,
    /// Head of the skeleton train, metres along `line`.
    head: f64,
    real_s: f64,
}

fn bounds<'a>(pts: impl Iterator<Item = &'a (f64, f64)>) -> (f64, f64, f64, f64) {
    pts.fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |(a, b, c, d), p| (a.min(p.0), b.max(p.0), c.min(p.1), d.max(p.1)))
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<App, JsValue> {
        install_panic_hook();
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: HtmlCanvasElement = doc
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("no element #{canvas_id}")))?
            .dyn_into()
            .map_err(|_| JsValue::from_str(&format!("#{canvas_id} is not a canvas")))?;
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d context unavailable"))?
            .dyn_into()?;
        let yard = Yard::parse(cabin_sim::NDLS).map_err(|e| JsValue::from_str(&format!("data/ndls.yard: {}", e.0)))?;
        let line = Line::along(&yard, &SKELETON_PATH).map_err(|e| JsValue::from_str(&e))?;
        let panel = Panel::new(&yard);
        let shapes: Vec<Vec<(f64, f64)>> = yard.edges.iter().map(|e| panel.map_poly(&e.poly)).collect();
        let all = bounds(shapes.iter().flatten());
        let running = bounds(yard.edges.iter().zip(&shapes).filter(|(e, _)| e.kind == Kind::Running).flat_map(|(_, s)| s));
        let cam = Camera { cx: 0.0, cy: 0.0, vx: 0.0, vy: 0.0, z: 0.0, zt: 0.0, vz: 0.0, pivot: None, raw: None, fitted: false };
        Ok(App { yard, panel, shapes, all, running, canvas, ctx, cam, w: 1.0, h: 1.0, dpr: 1.0, line, head: 0.0, real_s: 0.0 })
    }

    /// One animation frame: move the skeleton train by `real_ms`, run the
    /// camera springs, draw. Colours come from the page's CSS custom
    /// properties, so both themes are drawn by the same code.
    pub fn frame(&mut self, real_ms: f64, dpr: f64, reduce: bool, bg: &str, fg: &str, dim: &str, line: &str) {
        self.dpr = dpr;
        self.w = f64::from(self.canvas.width()) / dpr;
        self.h = f64::from(self.canvas.height()) / dpr;
        if !self.cam.fitted {
            self.fit();
        }
        // A hidden tab sends one long frame; a tenth of a second at most.
        let dt = (real_ms / 1000.0).clamp(0.0, 0.1);
        self.real_s += dt;
        self.head = (self.head + SKELETON_SPEED_MS * dt) % (self.line.len() + SKELETON_LEN_M);
        self.spring(dt, reduce);
        self.draw(bg, fg, dim, line);
    }

    pub fn osm_base(&self) -> String {
        self.yard.osm_base.clone()
    }

    pub fn real_s(&self) -> f64 {
        self.real_s
    }

    pub fn stretch(&self) -> f64 {
        PANEL_STRETCH
    }

    // ---- Input -------------------------------------------------------------

    /// Drag: the map follows the pointer 1:1 inside the yard and on a rubber
    /// band past its edges, CSS px.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let s = self.cam.scale();
        let (rx, ry) = self.cam.raw.unwrap_or((self.cam.cx, self.cam.cy));
        let (rx, ry) = (rx - dx / s, ry + dy / s);
        self.cam.raw = Some((rx, ry));
        let (u0, u1, v0, v1) = self.all;
        self.cam.cx = band(rx, u0, u1, self.w / s);
        self.cam.cy = band(ry, v0, v1, self.h / s);
        self.cam.vx = 0.0;
        self.cam.vy = 0.0;
        // Keep a running zoom pivoting on the same screen point.
        if let Some((sx, sy, _, _)) = self.cam.pivot {
            let (wx, wy) = self.to_panel(sx, sy);
            self.cam.pivot = Some((sx, sy, wx, wy));
        }
    }

    /// The drag ended: a centre past an edge springs back inside.
    pub fn release(&mut self) {
        self.cam.raw = None;
    }

    /// Wheel at (x, y) CSS px. Retargets the spring; never queues.
    pub fn wheel(&mut self, x: f64, y: f64, delta_y: f64) {
        let dz = (-delta_y.clamp(-200.0, 200.0)) * ZOOM_PER_WHEEL_PX;
        self.cam.zt = (self.cam.zt + dz).clamp(ZOOM_MIN.ln(), ZOOM_MAX.ln());
        let (wx, wy) = self.to_panel(x, y);
        self.cam.pivot = Some((x, y, wx, wy));
    }

    // ---- Test hooks (read-only) ----------------------------------------------

    pub fn zoom(&self) -> f64 {
        self.cam.scale()
    }

    /// [x, y] of the camera centre in panel metres.
    pub fn centre(&self) -> Vec<f64> {
        vec![self.cam.cx, self.cam.cy]
    }

    /// [u0, u1, v0, v1]: the panel bounds of everything drawn, metres.
    pub fn panel_bounds(&self) -> Vec<f64> {
        let (a, b, c, d) = self.all;
        vec![a, b, c, d]
    }

    /// [x, y] in CSS px of the skeleton train's head.
    pub fn train_screen(&self) -> Vec<f64> {
        let (x, y) = self.line.point(self.head);
        let (u, v) = self.panel.map(x, y);
        let (sx, sy) = self.to_screen(u, v);
        vec![sx, sy]
    }

    pub fn train_head_m(&self) -> f64 {
        self.head
    }

    /// [x, y] in CSS px of platform `i`'s midpoint.
    pub fn platform_screen(&self, i: usize) -> Vec<f64> {
        let Some(p) = self.yard.platforms.get(i) else { return vec![] };
        let s = &self.shapes[p.edge];
        let (sx, sy) = self.to_screen(s[s.len() / 2].0, s[s.len() / 2].1);
        vec![sx, sy]
    }

    pub fn platform_count(&self) -> usize {
        self.yard.platforms.len()
    }

    pub fn edge_count(&self) -> usize {
        self.yard.edges.len()
    }
}

impl App {
    fn to_screen(&self, u: f64, v: f64) -> (f64, f64) {
        let s = self.cam.scale();
        // Panel v grows up the screen; canvas y grows down.
        (self.w / 2.0 + (u - self.cam.cx) * s, self.h / 2.0 - (v - self.cam.cy) * s)
    }

    fn to_panel(&self, sx: f64, sy: f64) -> (f64, f64) {
        let s = self.cam.scale();
        (self.cam.cx + (sx - self.w / 2.0) / s, self.cam.cy - (sy - self.h / 2.0) / s)
    }

    /// Frame the running lines, once, when the canvas size is first known.
    fn fit(&mut self) {
        let (u0, u1, v0, v1) = self.running;
        self.cam.cx = (u0 + u1) / 2.0;
        self.cam.cy = (v0 + v1) / 2.0;
        let s = ((self.w / (u1 - u0)).min(self.h / (v1 - v0)) * FIT_SHARE).clamp(ZOOM_MIN, ZOOM_MAX);
        self.cam.z = s.ln();
        self.cam.zt = self.cam.z;
        self.cam.fitted = self.w > 1.0 && self.h > 1.0;
    }

    /// Advance the zoom spring and, once no drag or zoom holds it, the edge
    /// spring. Reduced motion jumps to each target.
    fn spring(&mut self, dt: f64, reduce: bool) {
        let c = &mut self.cam;
        if reduce {
            c.z = c.zt;
            c.vz = 0.0;
        } else {
            // Fixed 4 ms substeps keep semi-implicit Euler stable at any frame rate.
            let mut left = dt;
            while left > 0.0 {
                let h = left.min(0.004);
                c.vz += (ZOOM_K * (c.zt - c.z) - ZOOM_C * c.vz) * h;
                c.z += c.vz * h;
                left -= h;
            }
            if (c.zt - c.z).abs() < 1e-4 && c.vz.abs() < 1e-3 {
                c.z = c.zt;
                c.vz = 0.0;
            }
        }
        if let Some((sx, sy, wx, wy)) = c.pivot {
            let s = c.z.exp();
            c.cx = wx - (sx - self.w / 2.0) / s;
            c.cy = wy + (sy - self.h / 2.0) / s;
            if c.z == c.zt {
                c.pivot = None;
            }
            return;
        }
        if c.raw.is_some() {
            return;
        }
        let (u0, u1, v0, v1) = self.all;
        let (tx, ty) = (c.cx.clamp(u0, u1), c.cy.clamp(v0, v1));
        if reduce {
            (c.cx, c.cy, c.vx, c.vy) = (tx, ty, 0.0, 0.0);
            return;
        }
        let mut left = dt;
        while left > 0.0 {
            let h = left.min(0.004);
            c.vx += (EDGE_K * (tx - c.cx) - EDGE_C * c.vx) * h;
            c.vy += (EDGE_K * (ty - c.cy) - EDGE_C * c.vy) * h;
            c.cx += c.vx * h;
            c.cy += c.vy * h;
            left -= h;
        }
        // Settled within a hundredth of a CSS px: snap, so the hooks read
        // an exact edge.
        let px = 0.01 / c.z.exp();
        if (tx - c.cx).abs() < px && (ty - c.cy).abs() < px {
            (c.cx, c.cy, c.vx, c.vy) = (tx, ty, 0.0, 0.0);
        }
    }

    fn stroke(&self, pts: &[(f64, f64)]) {
        let ctx = &self.ctx;
        ctx.begin_path();
        for (k, &(u, v)) in pts.iter().enumerate() {
            let (x, y) = self.to_screen(u, v);
            if k == 0 {
                ctx.move_to(x, y);
            } else {
                ctx.line_to(x, y);
            }
        }
        ctx.stroke();
    }

    /// Rule A7: red, yellow and green are signal aspects and appear nowhere
    /// else, so this palette is ink only. Running track is dim, yard and
    /// siding track dimmer; B6 draws a set route bright.
    fn draw(&self, bg: &str, fg: &str, dim: &str, line: &str) {
        let ctx = &self.ctx;
        let _ = ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0);
        ctx.set_fill_style_str(bg);
        ctx.fill_rect(0.0, 0.0, self.w, self.h);
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        for pass in [false, true] {
            ctx.set_stroke_style_str(if pass { dim } else { line });
            ctx.set_line_width(if pass { 2.0 } else { 1.0 });
            for (e, s) in self.yard.edges.iter().zip(&self.shapes) {
                if (e.kind == Kind::Running) == pass {
                    self.stroke(s);
                }
            }
        }

        ctx.set_font("11px ui-monospace, \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace");
        ctx.set_text_baseline("middle");
        ctx.set_text_align("center");
        ctx.set_fill_style_str(dim);
        for p in &self.yard.platforms {
            let s = &self.shapes[p.edge];
            let (x, y) = self.to_screen(s[s.len() / 2].0, s[s.len() / 2].1);
            let _ = ctx.fill_text(&format!("P{}", p.name), x, y - 9.0);
        }
        for p in &self.yard.portals {
            let n = &self.yard.nodes[p.node];
            let (u, v) = self.panel.map(n.x, n.y);
            let (x, y) = self.to_screen(u, v);
            // North portals sit at the left end, south at the right; the
            // label goes outside the yard.
            let dx = if u < (self.all.0 + self.all.1) / 2.0 { -16.0 } else { 16.0 };
            let _ = ctx.fill_text(&p.name, x + dx, y);
        }

        // The skeleton train: a solid block with its number, ink only.
        let tail = (self.head - SKELETON_LEN_M).max(0.0);
        let head = self.head.min(self.line.len());
        if head > tail {
            let body: Vec<(f64, f64)> = densify(&self.line.piece(tail, head), DENSIFY_M).into_iter().map(|(x, y)| self.panel.map(x, y)).collect();
            ctx.set_stroke_style_str(fg);
            ctx.set_line_width(7.0);
            ctx.set_line_cap("butt");
            self.stroke(&body);
            let (u, v) = body[body.len() - 1];
            let (x, y) = self.to_screen(u, v);
            ctx.set_fill_style_str(fg);
            let _ = ctx.fill_text("TEST", x, y - 14.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ndls() -> Yard {
        Yard::parse(cabin_sim::NDLS).expect("baked yard parses")
    }

    #[test]
    fn skeleton_path_chains_from_portal_n1_to_platform_1() {
        let y = ndls();
        let line = Line::along(&y, &SKELETON_PATH).expect("path chains");
        let n1 = y.portals.iter().find(|p| p.name == "N1").expect("portal N1");
        let start = &y.nodes[n1.node];
        assert!((line.pts[0].0 - start.x).hypot(line.pts[0].1 - start.y) < 0.5, "starts at N1");
        let p1 = y.platforms.iter().find(|p| p.name == "1").expect("platform 1");
        assert_eq!(*SKELETON_PATH.last().unwrap(), p1.edge, "ends on platform 1's track");
        assert!(SKELETON_PATH.iter().all(|&i| y.edges[i].kind == Kind::Running));
    }

    #[test]
    fn panel_keeps_track_order_and_lays_the_station_along_u() {
        let y = ndls();
        let p = Panel::new(&y);
        // Monotonic across: for a fixed u, a point further left of the axis
        // maps further up. Checked on a grid over the whole extract.
        for ui in -60..=60 {
            let u = f64::from(ui) * 25.0;
            let mut last = f64::MIN;
            for vi in -40..=40 {
                let v = f64::from(vi) * 15.0;
                let (x, yy) = (p.ox + u * p.cos - v * p.sin, p.oy + u * p.sin + v * p.cos);
                let (mu, mv) = p.map(x, yy);
                assert!((mu - u).abs() < 1e-6, "u is true scale");
                assert!(mv > last, "order kept at u={u}, v={v}");
                last = mv;
            }
        }
        // Platforms run along u before the stretch. A platform's edge runs on
        // into the throat, where the tracks fan out (P16 drifts 81 m across
        // in 525 m), so the bound is loose: within about 11 degrees.
        for pl in &y.platforms {
            let s = &y.edges[pl.edge].poly;
            let (a, b) = (p.rotate(s[0].0, s[0].1), p.rotate(s[s.len() - 1].0, s[s.len() - 1].1));
            assert!((b.1 - a.1).abs() < 0.2 * (b.0 - a.0).abs(), "platform {} lies along the axis", pl.name);
        }
    }

    #[test]
    fn rubber_band_gives_way_and_never_reaches_the_view_size() {
        assert_eq!(band(5.0, 0.0, 10.0, 100.0), 5.0);
        let past = band(-50.0, 0.0, 10.0, 100.0);
        assert!(past < 0.0 && past > -50.0 * RUBBER - 1e-9, "past the edge it moves less than the pointer: {past}");
        assert!(band(-1e9, 0.0, 10.0, 100.0) > -100.0);
    }
}
