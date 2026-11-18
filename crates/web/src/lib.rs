//! CABIN-WEB · BROWSER SHELL
//!   · App: the wasm object the page drives, one per canvas; it owns the
//!     sim `World` and steps it TIME_SCALE times real time
//!   · Schematic: the yard's track connections drawn as rows and 45° jogs
//!   · Camera: pan 1:1, rubber band past the yard's edges, zoom on a spring
//!     about the cursor; the default view frames the platforms
//!   · controls: click an entry signal, then an exit; the route is
//!     previewed between the clicks; Esc clears; a refusal is one line
//!   · rack and command rows: tab-separated text the page lays out
//!   · draw: tracks, set routes, trains, signal heads, labels
//!   · test hooks: for Playwright; all read-only except `skip`
//!
//! Thin on purpose: real frame time and input come in from the page's
//! JavaScript, draw calls go out to a Canvas 2D context. No game rule lives
//! here; if it would matter in a headless test, it belongs in `cabin-sim`.

use cabin_sim::consts::{SHIFT_LEN_S, SHIFT_START_S, TICK_HZ, TIME_SCALE};
use cabin_sim::interlock::Aspect;
use cabin_sim::route::{Exit, Lie, Res, RouteId, SignalId};
use cabin_sim::traffic::{Mode, NDLS_TRAFFIC, Profile};
use cabin_sim::yard::{Kind, SignalKind, Yard};
use cabin_sim::{Action, World};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

mod schematic;

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

/// Hit slop (UI card No. 12): a click picks the nearest target within
/// this many CSS px, so each target is 44 px across while drawn small.
const HIT_PX: f64 = 22.0;
/// A signal head stands this far left of its track, CSS px: Indian
/// signals stand left of the line, and the head must not hide the track.
const HEAD_OFF_PX: f64 = 8.0;
const HEAD_R_PX: f64 = 4.5;
/// Heads closer than this, CSS px, share a click target, so the hit-slop
/// card's 44 px plus a pixel of margin. Two Starters 10 m before one
/// junction stood 3 px apart: no stretch separates them, so a crowded head
/// slides back along its own track, HEAD_STEP_M at a time, and a stalk
/// joins it to the post. The sim and the train still stop at the post.
const HEAD_GAP_PX: f64 = 45.0;
const HEAD_STEP_M: f64 = 2.0;
const HEAD_PASSES: usize = 4;
/// A point's leg that is not lying is broken for this many CSS px from the
/// switch (research/panel.md PN-04, PN-05: assumed). Cut out of the stroke,
/// not painted over, so the gap never eats the leg that lies.
const LIE_GAP_PX: f64 = 6.0;

/// The panel's colours, in the order the page reads them from CSS custom
/// properties (`PALETTE` in web/shell.html). This file names no colour:
/// both themes, the indications and the aspects all come from the page.
struct Palette {
    bg: String,
    fg: String,
    dim: String,
    /// Track, unlit when clear (PN-01).
    trk: String,
    /// A set and locked route (PN-02, PN-08).
    route: String,
    /// Occupied track, over any route (PN-03).
    occ: String,
    /// Signal aspects: red, yellow, green.
    asp: [String; 3],
}

impl Palette {
    fn new(p: Vec<String>) -> Result<Palette, JsValue> {
        let Ok([bg, fg, dim, trk, route, occ, r, y, g]) = <[String; 9]>::try_from(p) else {
            return Err(JsValue::from_str("palette: expected 9 colours: bg fg dim trk route occ asp-r asp-y asp-g"));
        };
        Ok(Palette { bg, fg, dim, trk, route, occ, asp: [r, y, g] })
    }
}

fn poly_len(poly: &[(f64, f64)]) -> f64 {
    poly.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum()
}
/// A place on the panel and the unit direction faced there.
type Place = ((f64, f64), (f64, f64));

/// The point `m` metres along `poly` from its first vertex, and the unit
/// direction there, with the edge's stated length `len` mapped onto the
/// polyline's own length (the two differ by rounding in the yard file).
fn at_metres(poly: &[(f64, f64)], len: f64, m: f64) -> ((f64, f64), (f64, f64)) {
    let total: f64 = poly.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
    let mut left = (m / len.max(1e-9) * total).clamp(0.0, total);
    for w in poly.windows(2) {
        let seg = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if seg <= 0.0 {
            continue;
        }
        let dir = ((w[1].0 - w[0].0) / seg, (w[1].1 - w[0].1) / seg);
        if left <= seg {
            return ((w[0].0 + dir.0 * left, w[0].1 + dir.1 * left), dir);
        }
        left -= seg;
    }
    let n = poly.len();
    let (a, b) = (poly[n.saturating_sub(2)], poly[n - 1]);
    let seg = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
    (b, ((b.0 - a.0) / seg, (b.1 - a.1) / seg))
}

/// The part of `poly` from `lo` to `hi` metres along it, `len` as above.
fn piece(poly: &[(f64, f64)], len: f64, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let total: f64 = poly.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
    let k = total / len.max(1e-9);
    let (lo, hi) = (lo * k, hi * k);
    let mut out = vec![at_metres(poly, total, lo).0];
    let mut at = 0.0;
    for w in poly.windows(2) {
        at += (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if at > lo && at < hi {
            out.push(w[1]);
        }
    }
    out.push(at_metres(poly, total, hi).0);
    out
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
    world: World,
    seed: u32,
    schematic: schematic::Schematic,
    /// Each edge's shape in panel metres, mapped once at load.
    shapes: Vec<Vec<(f64, f64)>>,
    /// Panel bounds of everything drawn, and of the platform tracks.
    all: (f64, f64, f64, f64),
    platforms: (f64, f64, f64, f64),
    /// Per signal: where its head may stand, as a panel position and the
    /// unit direction it faces. Index 0 is the post; each next one is
    /// HEAD_STEP_M further back along its own track.
    sig_path: Vec<Vec<Place>>,
    /// Per signal: the index into `sig_path` its head is drawn at; chosen by
    /// `place_heads` when the default view is framed.
    sig_pick: Vec<usize>,
    /// Per signal, the routes it is the entry of.
    from: Vec<Vec<RouteId>>,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    cam: Camera,
    w: f64,
    h: f64,
    dpr: f64,
    /// Ticks owed to the sim: real time times TIME_SCALE, carried between
    /// frames so no fraction of a tick is lost.
    owed: f64,
    paused: bool,
    /// The entry signal clicked first, and the route under the pointer.
    sel: Option<SignalId>,
    preview: Option<RouteId>,
    /// The line under the panel, whether it is a refusal, and a counter the
    /// page compares to know when to redraw it.
    status: String,
    refused: bool,
    status_seq: u32,
}

fn bounds<'a>(pts: impl Iterator<Item = &'a (f64, f64)>) -> (f64, f64, f64, f64) {
    pts.fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |(a, b, c, d), p| (a.min(p.0), b.max(p.0), c.min(p.1), d.max(p.1)))
}

fn mmss(s: u64) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

/// The sim's constants, read from their own source so the "what is real"
/// panel can never disagree with the build: `name\tvalue\ttag\treason`.
/// The tag is the first word of the doc comment, which `consts_tagged.rs`
/// demands is `solid` or `assumed`.
fn constant_rows() -> Vec<String> {
    let src = include_str!("../../sim/src/consts.rs");
    let mut rows = Vec::new();
    let mut doc: Vec<&str> = Vec::new();
    for l in src.lines().map(str::trim) {
        if let Some(d) = l.strip_prefix("///") {
            doc.push(d.trim());
        } else if let Some(rest) = l.strip_prefix("pub const ") {
            let (name, value) = rest.split_once(':').zip(rest.split_once('=')).map_or(("?", "?"), |((n, _), (_, v))| (n, v));
            let text = doc.join(" ");
            let (tag, why) = text.split_once(':').unwrap_or(("untagged", &text));
            rows.push(format!("{name}\t{}\t{tag}\t{}", value.trim().trim_end_matches(';'), why.trim()));
            doc.clear();
        } else {
            doc.clear();
        }
    }
    rows
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    /// `mode` is `practice`, `light`, `normal` or `peak`; the page checks
    /// `?mode=` and says so before it gets here.
    pub fn new(canvas_id: &str, seed: u32, mode: &str) -> Result<App, JsValue> {
        install_panic_hook();
        let mode = Mode::parse(mode).ok_or_else(|| JsValue::from_str(&format!("no mode '{mode}'")))?;
        let profile = Profile::parse(NDLS_TRAFFIC).map_err(|e| JsValue::from_str(&format!("data/ndls.traffic: {e}")))?;
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
        let schematic = schematic::Schematic::new(&yard);
        let shapes = schematic.shape.clone();
        let all = bounds(shapes.iter().flatten());
        let platforms = bounds(yard.platforms.iter().flat_map(|p| &shapes[p.edge]));
        let sig_path: Vec<Vec<_>> = yard
            .signals
            .iter()
            .map(|s| {
                let e = &yard.edges[s.edge];
                // Back is away from the way the signal faces, to the edge's end.
                let room = if s.forward { s.offset } else { e.len - s.offset };
                let back = if s.forward { -HEAD_STEP_M } else { HEAD_STEP_M };
                (0..=(room.max(0.0) / HEAD_STEP_M) as usize)
                    .map(|k| {
                        let (p, d) = at_metres(&shapes[s.edge], e.len, s.offset + back * k as f64);
                        let d = if s.forward { d } else { (-d.0, -d.1) };
                        (p, d)
                    })
                    .collect()
            })
            .collect();
        let sig_pick = vec![0; sig_path.len()];
        let world = World::shift(yard, profile, mode, u64::from(seed));
        let mut from = vec![Vec::new(); world.yard.signals.len()];
        for r in world.lock.routes() {
            from[r.entry].push(r.id);
        }
        let cam = Camera { cx: 0.0, cy: 0.0, vx: 0.0, vy: 0.0, z: 0.0, zt: 0.0, vz: 0.0, pivot: None, raw: None, fitted: false };
        Ok(App {
            world,
            seed,
            schematic,
            shapes,
            all,
            platforms,
            sig_path,
            sig_pick,
            from,
            canvas,
            ctx,
            cam,
            w: 1.0,
            h: 1.0,
            dpr: 1.0,
            owed: 0.0,
            paused: false,
            sel: None,
            preview: None,
            status: String::new(),
            refused: false,
            status_seq: 0,
        })
    }

    /// One animation frame: step the sim by `real_ms` at TIME_SCALE, run
    /// the camera springs, draw. `palette` is the page's CSS custom
    /// properties, see `Palette`, so both themes are drawn by the same code.
    pub fn frame(&mut self, real_ms: f64, dpr: f64, reduce: bool, palette: Vec<String>) -> Result<(), JsValue> {
        let pal = Palette::new(palette)?;
        self.dpr = dpr;
        self.w = f64::from(self.canvas.width()) / dpr;
        self.h = f64::from(self.canvas.height()) / dpr;
        if !self.cam.fitted {
            self.fit();
        }
        // A hidden tab sends one long frame; a tenth of a second at most.
        let dt = (real_ms / 1000.0).clamp(0.0, 0.1);
        if !self.paused {
            self.owed += dt * TIME_SCALE * f64::from(TICK_HZ);
            let n = self.owed.floor();
            self.owed -= n;
            self.world.step_ticks(n as u32);
        }
        self.spring(dt, reduce);
        self.draw(&pal);
        Ok(())
    }

    pub fn osm_base(&self) -> String {
        self.world.yard.osm_base.clone()
    }

    /// The "what is real" panel, one row per layer: `layer\ttag\tclaim`.
    /// Counts and dates come from the baked files, so they cannot go stale.
    pub fn real(&self) -> Vec<String> {
        let y = &self.world.yard;
        let homes = y.signals.iter().filter(|s| s.kind == SignalKind::Home).count();
        let (p, written) = self.world.profile.as_ref().map_or((0, "unknown"), |p| (p.hourly.iter().sum::<u32>(), p.written.as_str()));
        let pool = self.world.profile.as_ref().map_or(0, |p| p.pool.len());
        [
            ("Rules", "primary", "Each check the interlocking makes cites a General Rules row of rails/research.md (C3-133 to C3-164). cargo test fails if a cited row no longer says what the game says it does.".to_string()),
            ("Speeds", "assumed", "50 km/h, and 15 km/h over points set reverse, are Southern Railway's figures (C3-52, C3-49). NDLS is on Northern Railway, whose Subsidiary Rules were not read.".to_string()),
            ("Track", "OpenStreetMap", format!("Extract of {}: {} track sections, {} points, {} crossings, {} platforms, {} portals. Not checked against a track diagram: none was found.", y.osm_base.get(..10).unwrap_or(&y.osm_base), y.edges.len(), y.points.len(), y.crossings.len(), y.platforms.len(), y.portals.len())),
            ("Promoted track", "assumed", format!("{} yard or siding ways are worked as running lines, promoted by rule so that trains can reach the platforms.", y.promoted.len())),
            ("Slips", "assumed", "A switch-tagged node where two tracks cross is read as a double slip. OpenStreetMap does not say single or double.".to_string()),
            ("Platform numbers", "assumed", "Assigned by side, ascending west to east.".to_string()),
            ("Signals", "authored", format!("{} signals ({homes} Home, {} Starter), all placed by rule (C3-58, C3-63). None comes from OpenStreetMap or a diagram.", y.signals.len(), y.signals.len() - homes)),
            ("Routes", "authored", format!("{} routes: the shortest path for each pair of signals. Arrivals into a platform with no way out are removed.", self.world.lock.routes().len())),
            ("Aspects", "assumed", "Red with no route, yellow when the exit signal is red, green otherwise. The ledger does not settle a Starter's off aspect.".to_string()),
            ("Traffic", "secondary", format!("About {p} trains on a busy day, from a 2016 survey of the station's berthing chart (AG18) and the datameet 2016 timetable, read {written}. No figure is from Indian Railways. The {pool} train numbers and names are real; their times are drawn from the seed.")),
            ("Score and shift", "assumed", "Three plain numbers, never blended. The shift length, its start and the mode fractions were chosen by you on 2026-11-18; see the constants below.".to_string()),
            ("Panel", "schematic", "Track order and connections are the yard's; lengths and angles are drawn, not measured.".to_string()),
            ("Indications", "primary", "Clear track unlit, a set and locked route white, occupied track red over any route: the Indian Railways track-circuit chapter (2023) section 17.2.6 and Signal Engineering Manual section 7.172.2 (research/panel.md PN-01 to PN-03, PN-08). On the light theme the route is black.".to_string()),
            ("Point lie", "assumed", "A point's leg that is not lying is drawn broken at the switch. A real panel shows the lie, but how it looks was not found (PN-04, PN-05).".to_string()),
        ]
        .into_iter()
        .map(|(a, b, c)| format!("{a}\t{b}\t{c}"))
        .collect()
    }

    /// Every constant of the sim with its value and tag; see `constant_rows`.
    pub fn constants(&self) -> Vec<String> {
        constant_rows()
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn time_scale(&self) -> f64 {
        TIME_SCALE
    }

    /// Game seconds since the shift began.
    pub fn game_s(&self) -> f64 {
        self.world.tick as f64 / f64::from(TICK_HZ)
    }

    /// The game clock, seconds after midnight.
    pub fn clock_s(&self) -> f64 {
        self.world.clock_s() as f64
    }

    pub fn mode(&self) -> String {
        self.world.mode.map_or("", Mode::name).to_string()
    }

    /// The shift's start and end on the game clock, seconds after midnight.
    pub fn shift_window(&self) -> Vec<f64> {
        vec![f64::from(SHIFT_START_S), f64::from(SHIFT_START_S + SHIFT_LEN_S)]
    }

    /// The three numbers and whether the shift is over: `[handled, delay
    /// in game minutes per train due, put-backs, over 0 or 1]`. Practice
    /// keeps them at 0.
    pub fn score(&self) -> Vec<f64> {
        let s = &self.world.score;
        vec![f64::from(s.handled), s.delay_per_train(), f64::from(s.put_backs), f64::from(u8::from(s.over.is_some()))]
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
    }

    // ---- Camera input ------------------------------------------------------

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

    // ---- Controls ----------------------------------------------------------

    /// A click on the panel at (x, y) CSS px. With an entry chosen, an exit
    /// of it sets the route; otherwise a signal that sets routes becomes
    /// the entry; a click on nothing clears the choice.
    pub fn click(&mut self, x: f64, y: f64) {
        if let Some(r) = self.exit_at(x, y) {
            self.set_route(r);
            return;
        }
        match self.entry_at(x, y) {
            Some(s) if self.sel == Some(s) => self.clear(),
            Some(s) => self.choose(s),
            None => self.clear(),
        }
    }

    /// The pointer moved to (x, y): preview the route to the exit under it.
    /// True when a target is under it, for the cursor.
    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.preview = self.exit_at(x, y);
        self.preview.is_some() || self.entry_at(x, y).is_some()
    }

    /// Preview a route from the page's exit buttons; a negative id clears.
    pub fn preview_route(&mut self, id: i32) {
        self.preview = usize::try_from(id).ok().filter(|&r| Some(self.world.lock.routes().get(r).map(|r| r.entry)) == Some(self.sel));
    }

    /// Choose an entry by name, from the rack or the command menu.
    pub fn select(&mut self, name: &str) -> bool {
        match self.world.yard.signals.iter().position(|s| s.name == name) {
            Some(s) if !self.from[s].is_empty() => {
                self.choose(s);
                true
            }
            _ => false,
        }
    }

    /// Esc: drop the chosen entry. False when nothing was chosen.
    pub fn escape(&mut self) -> bool {
        let had = self.sel.is_some();
        self.clear();
        had
    }

    pub fn set_route(&mut self, id: usize) {
        let Some(r) = self.world.lock.routes().get(id) else { return };
        let a = Action::SetRoute { entry: r.entry, exit: r.exit };
        match self.world.apply(a) {
            Ok(()) => {
                let msg = format!("{} set.", self.world.route_name(id));
                self.sel = None;
                self.preview = None;
                self.say(msg, false);
            }
            Err(e) => {
                let msg = self.world.describe(a, e);
                self.say(msg, true);
            }
        }
    }

    pub fn cancel_route(&mut self, id: usize) {
        if id >= self.world.lock.routes().len() {
            return;
        }
        let a = Action::CancelRoute(id);
        match self.world.apply(a) {
            Ok(()) => {
                let name = self.world.route_name(id);
                // A train on the approach keeps the locks for a while.
                let msg = if self.world.lock.live().contains(&id) {
                    format!("{name} put back; approach locked, a train may still run through.")
                } else {
                    format!("{name} cancelled.")
                };
                self.sel = None;
                self.say(msg, false);
            }
            Err(e) => {
                let msg = self.world.describe(a, e);
                self.say(msg, true);
            }
        }
    }

    /// Cancel the route set from the chosen entry, if there is one.
    pub fn cancel_selected(&mut self) -> bool {
        match self.sel.and_then(|s| self.set_from(s)) {
            Some(r) => {
                self.cancel_route(r);
                true
            }
            None => false,
        }
    }

    pub fn accept(&mut self, portal: usize) {
        if portal >= self.world.yard.portals.len() {
            return;
        }
        let a = Action::Accept(portal);
        match self.world.apply(a) {
            Ok(()) => {
                // Ids only grow, so the train just accepted is the last.
                let t = *self.world.trains.keys().next_back().expect("an accepted train");
                let home = self.world.trains[&t].next_signal.map_or(String::new(), |s| self.world.yard.signals[s].name.clone());
                let msg = format!("{} accepted at {}; it runs to {home}.", self.world.train_name(t), self.world.yard.portals[portal].name);
                self.say(msg, false);
            }
            Err(e) => {
                let msg = self.world.describe(a, e);
                self.say(msg, true);
            }
        }
    }

    pub fn status(&self) -> String {
        self.status.clone()
    }

    pub fn status_refused(&self) -> bool {
        self.refused
    }

    pub fn status_seq(&self) -> u32 {
        self.status_seq
    }

    /// The chosen entry's name, or "".
    pub fn selected(&self) -> String {
        self.sel.map_or(String::new(), |s| self.world.yard.signals[s].name.clone())
    }

    /// The chosen entry's routes, one row each: `id \t exit \t set`, where
    /// `set` is 1 for the route set now. The page lays them out as buttons.
    pub fn exits(&self) -> Vec<String> {
        let Some(s) = self.sel else { return Vec::new() };
        let set = self.set_from(s);
        self.from[s]
            .iter()
            .map(|&r| format!("{r}\t{}\t{}", self.world.exit_name(self.world.lock.routes()[r].exit), u8::from(set == Some(r))))
            .collect()
    }

    /// The rack, one row per line: `bay \t key \t name \t text \t action \t
    /// arg \t title`. Bays are `due`, `platform`, `depart`. Actions: `accept`
    /// (arg: portal id), `route` (arg: signal name), or empty. `title` is
    /// the train's full name, or "".
    pub fn rack(&self) -> Vec<String> {
        let w = &self.world;
        let y = &w.yard;
        let hz = u64::from(TICK_HZ);
        let mut rows = Vec::new();
        for (&p, q) in &w.offers {
            if let Some(o) = q.front() {
                let behind = if q.len() > 1 { format!(", {} more behind", q.len() - 1) } else { String::new() };
                let (number, title) = w.offer_name(o);
                let number = if number.is_empty() { "Offer".to_string() } else { number };
                rows.push(format!("due\to{p}\t{number}\toffered at {}, waiting {}{behind}\taccept\t{p}\t{title}", y.portals[p].name, mmss((w.tick - o.since) / hz)));
            }
        }
        for (&id, t) in &w.trains {
            let name = w.train_name(id);
            let sig = |s: SignalId| y.signals[s].name.clone();
            let plat = |s: SignalId| y.signals[s].platform.map_or(String::new(), |k| format!("P{} ", y.platforms[k].name));
            let row = match (t.leaving, t.next_signal) {
                (Some(p), _) => format!("depart\tt{id}\t{name}\tleaving by {}\t\t", y.portals[p].name),
                (None, Some(s)) if y.signals[s].kind == SignalKind::Home => {
                    // Set but not yet taken: the train moves on the next tick.
                    let need = if w.lock.takeable(s).is_some() { "route set" } else { "needs a route" };
                    format!("due\tt{id}\t{name}\tat {}, {need}\troute\t{}", sig(s), sig(s))
                }
                (None, Some(s)) => match t.dwell_until {
                    None => format!("due\tt{id}\t{name}\trunning in to {}({})\t\t", plat(s), sig(s)),
                    Some(u) if w.tick < u => format!("platform\tt{id}\t{name}\t{}dwell, {} left\troute\t{}", plat(s), mmss((u - w.tick).div_ceil(hz)), sig(s)),
                    Some(u) => format!("depart\tt{id}\t{name}\t{}ready {} ago, needs a route\troute\t{}", plat(s), mmss((w.tick - u) / hz), sig(s)),
                },
                (None, None) => continue,
            };
            rows.push(format!("{row}\t{}", w.train_title(id)));
        }
        rows
    }

    /// Commands for the menu: `kind \t id \t title`. Kinds: `route`,
    /// `cancel`, `accept`. Typing two names, such as `hn3 p5`, finds a route.
    pub fn commands(&self) -> Vec<String> {
        let w = &self.world;
        let mut out: Vec<String> = w.offers.iter().filter(|(_, q)| !q.is_empty()).map(|(&p, q)| format!("accept\t{p}\tAccept {} offered at {}", w.offer_name(&q[0]).0, w.yard.portals[p].name)).collect();
        out.extend(w.lock.live().into_iter().map(|r| format!("cancel\t{r}\tCancel {}", w.route_name(r))));
        out.extend((0..w.lock.routes().len()).map(|r| format!("route\t{r}\tSet {}", w.route_name(r))));
        out
    }

    // ---- Test hooks: read-only, except `skip` ------------------------------

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

    /// [x, y] in CSS px of platform `i`'s midpoint.
    pub fn platform_screen(&self, i: usize) -> Vec<f64> {
        let Some(p) = self.world.yard.platforms.get(i) else { return vec![] };
        let s = &self.shapes[p.edge];
        let (sx, sy) = self.to_screen(s[s.len() / 2].0, s[s.len() / 2].1);
        vec![sx, sy]
    }

    pub fn platform_count(&self) -> usize {
        self.world.yard.platforms.len()
    }

    pub fn edge_count(&self) -> usize {
        self.world.yard.edges.len()
    }

    pub fn signal_names(&self) -> Vec<String> {
        self.world.yard.signals.iter().map(|s| s.name.clone()).collect()
    }

    /// [x0, y0, x1, y1, ...] in CSS px: each signal head, in signal order.
    pub fn signal_screens(&self) -> Vec<f64> {
        (0..self.world.yard.signals.len()).flat_map(|s| {
            let (x, y) = self.head_screen(s);
            [x, y]
        }).collect()
    }

    /// [x, y] in CSS px of portal `name`'s end of track, or [] if none.
    pub fn portal_screen(&self, name: &str) -> Vec<f64> {
        let Some(p) = self.world.yard.portals.iter().position(|p| p.name == name) else { return vec![] };
        let (x, y) = self.portal_xy(p);
        vec![x, y]
    }

    pub fn portal_names(&self) -> Vec<String> {
        self.world.yard.portals.iter().map(|p| p.name.clone()).collect()
    }

    pub fn aspect_of(&self, name: &str) -> String {
        self.world.yard.signals.iter().position(|s| s.name == name).map_or(String::new(), |s| format!("{:?}", self.world.aspect(s)))
    }

    /// The route drawn as the preview, or -1.
    pub fn previewing(&self) -> i32 {
        self.preview.map_or(-1, |r| r as i32)
    }

    pub fn train_count(&self) -> usize {
        self.world.trains.len()
    }

    /// Routes that hold a lock now, by name.
    pub fn live_routes(&self) -> Vec<String> {
        self.world.lock.live().into_iter().map(|r| self.world.route_name(r)).collect()
    }

    /// The id of the route from `entry` to the exit named `exit` (`P5
    /// (ST5S)` or `N2`), or -1.
    pub fn route_id(&self, entry: &str, exit: &str) -> i32 {
        let w = &self.world;
        w.lock.routes().iter().find(|r| w.yard.signals[r.entry].name == entry && w.exit_name(r.exit) == exit).map_or(-1, |r| r.id as i32)
    }

    /// Test hook, read-only: route `id`'s path edges as drawn, in CSS px,
    /// `x, y` pairs with NaN, NaN between edges. verify_b9 samples the
    /// canvas along them to prove the drawn route has no gap.
    pub fn route_screen(&self, id: usize) -> Vec<f64> {
        let Some(route) = self.world.lock.routes().get(id) else { return Vec::new() };
        let mut out = Vec::new();
        for &e in &route.path {
            for &(u, v) in &self.shapes[e] {
                let (x, y) = self.to_screen(u, v);
                out.extend([x, y]);
            }
            out.extend([f64::NAN, f64::NAN]);
        }
        out
    }

    /// Test hook, read-only: the middle of each occupied edge as drawn, CSS
    /// px, `x, y` pairs. verify_b9 reads the occupied colour there.
    pub fn occupied_screen(&self) -> Vec<f64> {
        let occ = self.world.occupancy();
        let edges: HashSet<usize> = occ.bodies.values().flat_map(|b| b.spans.iter().map(|s| s.edge)).collect();
        let mut out = Vec::new();
        for e in edges {
            let s = &self.shapes[e];
            if s.len() < 2 {
                continue;
            }
            let t = poly_len(s);
            let ((u, v), _) = at_metres(s, t, t / 2.0);
            let (x, y) = self.to_screen(u, v);
            out.extend([x, y]);
        }
        out
    }

    /// Test hook, read-only: per drawn point, its index, then the place
    /// `px` CSS px from its switch along the leg not lying, then along the
    /// leg lying: five numbers.
    /// Points with a leg drawn shorter than twice `px` are left out.
    pub fn lie_screen(&self, px: f64) -> Vec<f64> {
        let y = &self.world.yard;
        let m = px / self.cam.scale();
        let mut out = Vec::new();
        for (k, p) in y.points.iter().enumerate() {
            let (off, on) = if self.world.lie(k) == Lie::Normal { (p.reverse, p.normal) } else { (p.normal, p.reverse) };
            let at = |e: usize| {
                let s = &self.shapes[e];
                let t = poly_len(s);
                (s.len() >= 2 && t >= 2.0 * m).then(|| {
                    let ((u, v), _) = at_metres(s, t, if y.edges[e].a == p.node { m } else { t - m });
                    self.to_screen(u, v)
                })
            };
            if let (Some(a), Some(b)) = (at(off), at(on)) {
                out.extend([k as f64, a.0, a.1, b.0, b.1]);
            }
        }
        out
    }

    /// Test hook: run `game_s` of game time at once, as the frame loop
    /// would over many frames. It steps the same sim; it changes no rule.
    pub fn skip(&mut self, game_s: f64) {
        self.world.step_ticks((game_s * f64::from(TICK_HZ)).round() as u32);
    }
}

impl App {
    fn say(&mut self, msg: String, refused: bool) {
        self.status = msg;
        self.refused = refused;
        self.status_seq += 1;
    }

    fn clear(&mut self) {
        self.sel = None;
        self.preview = None;
        self.say(String::new(), false);
    }

    fn choose(&mut self, s: SignalId) {
        self.sel = Some(s);
        self.preview = None;
        let name = self.world.yard.signals[s].name.clone();
        let msg = match self.set_from(s) {
            Some(r) => format!("{} is set. Delete cancels it; Esc clears.", self.world.route_name(r)),
            None => format!("{name}: click an exit, or Esc to clear."),
        };
        self.say(msg, false);
    }

    /// The route set from signal `s` that no train has passed the entry of.
    fn set_from(&self, s: SignalId) -> Option<RouteId> {
        let w = &self.world;
        w.lock.live().into_iter().find(|&r| w.lock.routes()[r].entry == s && !w.trains.values().any(|t| t.route == Some(r) && t.to_signal(&w.yard, s).is_none()))
    }

    fn to_screen(&self, u: f64, v: f64) -> (f64, f64) {
        let s = self.cam.scale();
        // Panel v grows up the screen; canvas y grows down.
        (self.w / 2.0 + (u - self.cam.cx) * s, self.h / 2.0 - (v - self.cam.cy) * s)
    }

    fn to_panel(&self, sx: f64, sy: f64) -> (f64, f64) {
        let s = self.cam.scale();
        (self.cam.cx + (sx - self.w / 2.0) / s, self.cam.cy - (sy - self.h / 2.0) / s)
    }

    /// A signal's head on screen: the place `place_heads` chose on its
    /// track, moved HEAD_OFF_PX to the left of the direction it faces.
    fn head_screen(&self, s: SignalId) -> (f64, f64) {
        self.head_xy(self.sig_path[s][self.sig_pick[s]])
    }

    fn head_xy(&self, ((u, v), (du, dv)): Place) -> (f64, f64) {
        let (x, y) = self.to_screen(u, v);
        // Screen direction is (du, -dv); its left is (-dv, -du).
        (x - dv * HEAD_OFF_PX, y - du * HEAD_OFF_PX)
    }

    /// Give each head the place nearest its post that stands HEAD_GAP_PX
    /// from every other head, at the current zoom. Run when the default view
    /// is framed: zooming in only spreads heads further. Each pass re-places
    /// every head against all the others, so a head placed early can still
    /// step back for one placed later.
    /// ponytail: HEAD_PASSES fixed passes, not a solver; a head with no
    /// clear place on its track takes the one farthest from the others.
    fn place_heads(&mut self) {
        let spots: Vec<Vec<(f64, f64)>> = self.sig_path.iter().map(|p| p.iter().map(|&c| self.head_xy(c)).collect()).collect();
        self.sig_pick.fill(0);
        for _ in 0..HEAD_PASSES {
            for s in 0..spots.len() {
                let pick = &self.sig_pick;
                let room = |p: (f64, f64)| {
                    (0..spots.len()).filter(|&o| o != s).map(|o| {
                        let q = spots[o][pick[o]];
                        (p.0 - q.0).hypot(p.1 - q.1)
                    }).fold(f64::INFINITY, f64::min)
                };
                let mine = &spots[s];
                let k = mine
                    .iter()
                    .position(|&p| room(p) >= HEAD_GAP_PX)
                    .unwrap_or_else(|| (0..mine.len()).max_by(|&a, &b| room(mine[a]).total_cmp(&room(mine[b]))).unwrap_or(0));
                self.sig_pick[s] = k;
            }
        }
    }

    fn portal_xy(&self, p: usize) -> (f64, f64) {
        let (u, v) = self.schematic.node[self.world.yard.portals[p].node]
            .expect("a portal is attached to a drawn track");
        self.to_screen(u, v)
    }

    fn exit_xy(&self, exit: Exit) -> (f64, f64) {
        match exit {
            Exit::Berth { starter, .. } => self.head_screen(starter),
            Exit::Portal(p) => self.portal_xy(p),
        }
    }

    /// The chosen entry's route whose exit is nearest (x, y), within HIT_PX.
    fn exit_at(&self, x: f64, y: f64) -> Option<RouteId> {
        let s = self.sel?;
        let routes = self.world.lock.routes();
        self.from[s]
            .iter()
            .map(|&r| {
                let (ex, ey) = self.exit_xy(routes[r].exit);
                (r, (ex - x).hypot(ey - y))
            })
            .filter(|&(_, d)| d <= HIT_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(r, _)| r)
    }

    /// The signal that sets routes nearest (x, y), within HIT_PX.
    fn entry_at(&self, x: f64, y: f64) -> Option<SignalId> {
        (0..self.sig_path.len())
            .filter(|&s| !self.from[s].is_empty())
            .map(|s| {
                let (hx, hy) = self.head_screen(s);
                (s, (hx - x).hypot(hy - y))
            })
            .filter(|&(_, d)| d <= HIT_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(s, _)| s)
    }

    /// The default view frames the platform tracks, once, when the canvas
    /// size is first known; pan to see the throats beyond the platform view.
    fn fit(&mut self) {
        let (u0, u1, v0, v1) = self.platforms;
        self.cam.cx = (u0 + u1) / 2.0;
        self.cam.cy = (v0 + v1) / 2.0;
        let s = ((self.w / (u1 - u0)).min(self.h / (v1 - v0)) * FIT_SHARE).clamp(ZOOM_MIN, ZOOM_MAX);
        self.cam.z = s.ln();
        self.cam.zt = self.cam.z;
        self.cam.fitted = self.w > 1.0 && self.h > 1.0;
        self.place_heads();
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

    fn ring(&self, x: f64, y: f64, r: f64) {
        self.ctx.begin_path();
        let _ = self.ctx.arc(x, y, r, 0.0, std::f64::consts::TAU);
        self.ctx.stroke();
    }

    /// Aspect colours come from the palette. Each head has an ink outline,
    /// so yellow on the light theme still reads as a shape.
    fn draw_heads(&self, p: &Palette) {
        let ctx = &self.ctx;
        ctx.set_line_width(1.0);
        ctx.set_stroke_style_str(&p.fg);
        for s in 0..self.sig_path.len() {
            let (x, y) = self.head_screen(s);
            // A head slid back from a crowded post: a stalk to the post.
            if self.sig_pick[s] > 0 {
                let (px, py) = self.head_xy(self.sig_path[s][0]);
                ctx.begin_path();
                ctx.move_to(px, py);
                ctx.line_to(x, y);
                ctx.stroke();
            }
            ctx.set_fill_style_str(match self.world.aspect(s) {
                Aspect::Red => &p.asp[0],
                Aspect::Yellow | Aspect::DoubleYellow => &p.asp[1],
                Aspect::Green => &p.asp[2],
            });
            ctx.begin_path();
            let _ = ctx.arc(x, y, HEAD_R_PX, 0.0, std::f64::consts::TAU);
            ctx.fill();
            ctx.stroke();
        }
    }

    /// Where each edge's stroke is cut short, CSS px at its `a` and `b`
    /// ends: the leg of each point that is not lying, at the switch.
    fn lie_cuts(&self) -> Vec<(f64, f64)> {
        let y = &self.world.yard;
        let mut cut = vec![(0.0, 0.0); y.edges.len()];
        for (k, p) in y.points.iter().enumerate() {
            let off = if self.world.lie(k) == Lie::Normal { p.reverse } else { p.normal };
            if y.edges[off].a == p.node {
                cut[off].0 = LIE_GAP_PX;
            } else {
                cut[off].1 = LIE_GAP_PX;
            }
        }
        cut
    }

    /// The Indian Railways panel convention (research/panel.md): clear track
    /// unlit, a set route in the route colour (path solid, overlap dashed),
    /// occupied track sections red over any route, each train's number in a
    /// describer box. Draw order is that override order.
    fn draw(&self, p: &Palette) {
        let ctx = &self.ctx;
        let w = &self.world;
        let y = &w.yard;
        let _ = ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0);
        ctx.set_fill_style_str(&p.bg);
        ctx.fill_rect(0.0, 0.0, self.w, self.h);
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        let scale = self.cam.scale();
        let cuts = self.lie_cuts();
        ctx.set_stroke_style_str(&p.trk);
        for pass in [false, true] {
            ctx.set_line_width(if pass { 2.0 } else { 1.0 });
            for ((e, s), &(c0, c1)) in y.edges.iter().zip(&self.shapes).zip(&cuts) {
                if (e.kind == Kind::Running) != pass || s.len() < 2 {
                    continue;
                }
                if c0 == 0.0 && c1 == 0.0 {
                    self.stroke(s);
                } else {
                    let t = poly_len(s);
                    let (lo, hi) = (c0 / scale, t - c1 / scale);
                    if hi > lo {
                        self.stroke(&piece(s, t, lo, hi));
                    }
                }
            }
        }
        ctx.set_line_width(1.0);
        for stub in &self.schematic.stubs {
            self.stroke(stub);
        }

        // Set routes: what the interlocking holds, solid where it is a live
        // route's path, dashed where it is only an overlap.
        let path: HashSet<usize> = w.lock.live().into_iter().flat_map(|r| w.lock.routes()[r].path.iter().copied()).collect();
        ctx.set_stroke_style_str(&p.route);
        ctx.set_line_width(3.0);
        for dashed in [false, true] {
            if dashed {
                let _ = ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(2.0), &JsValue::from(5.0)));
            }
            for (k, s) in self.shapes.iter().enumerate() {
                if w.lock.held(Res::Edge(k)) && path.contains(&k) != dashed {
                    self.stroke(s);
                }
            }
        }
        if let Some(r) = self.preview {
            let route = &w.lock.routes()[r];
            let _ = ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(6.0), &JsValue::from(5.0)));
            for &e in route.path.iter().chain(&route.overlap) {
                self.stroke(&self.shapes[e]);
            }
        }
        let _ = ctx.set_line_dash(&js_sys::Array::new());

        // Occupied: the whole track section, red over any route (PN-03).
        let occ = w.occupancy();
        let held: HashSet<usize> = occ.bodies.values().flat_map(|b| b.spans.iter().map(|s| s.edge)).collect();
        ctx.set_stroke_style_str(&p.occ);
        for &e in &held {
            self.stroke(&self.shapes[e]);
        }

        // Describers: each train's number boxed above its head.
        ctx.set_font("700 11px ui-monospace, \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.set_line_width(1.0);
        for (&id, b) in &occ.bodies {
            let e = &y.edges[b.head.0];
            let ((u, v), _) = at_metres(&self.shapes[b.head.0], e.len, b.head.1);
            let (x, yy) = self.to_screen(u, v);
            let name = w.train_name(id);
            // Monospace: 0.6 em a character, so no text measuring is needed.
            let half = name.chars().count() as f64 * 6.6 / 2.0 + 4.0;
            let top = yy - 24.0;
            ctx.set_fill_style_str(&p.bg);
            ctx.fill_rect(x - half, top, 2.0 * half, 15.0);
            ctx.set_stroke_style_str(&p.route);
            ctx.stroke_rect(x - half, top, 2.0 * half, 15.0);
            ctx.set_fill_style_str(&p.route);
            let _ = ctx.fill_text(&name, x, top + 8.0);
        }

        // Choice: the entry ringed solid, its exits ringed dashed, the one
        // under the pointer ringed solid.
        if let Some(s) = self.sel {
            ctx.set_stroke_style_str(&p.fg);
            ctx.set_line_width(2.0);
            let (x, yy) = self.head_screen(s);
            self.ring(x, yy, HIT_PX / 2.0);
            let _ = ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(3.0), &JsValue::from(3.0)));
            for &r in &self.from[s] {
                let (x, yy) = self.exit_xy(w.lock.routes()[r].exit);
                if self.preview != Some(r) {
                    self.ring(x, yy, HIT_PX / 2.0);
                }
            }
            let _ = ctx.set_line_dash(&js_sys::Array::new());
            if let Some(r) = self.preview {
                let (x, yy) = self.exit_xy(w.lock.routes()[r].exit);
                self.ring(x, yy, HIT_PX / 2.0);
            }
        }
        self.draw_heads(p);

        ctx.set_font("11px ui-monospace, \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace");
        ctx.set_fill_style_str(&p.dim);
        // Mid flat run, not mid vertex: a jogging platform's middle vertex is
        // its jog corner, where the Starter's head and name already sit.
        for p in &y.platforms {
            let s = &self.shapes[p.edge];
            let Some(run) = s.windows(2).filter(|w| w[0].1 == w[1].1)
                .max_by(|a, b| (a[1].0 - a[0].0).abs().total_cmp(&(b[1].0 - b[0].0).abs())) else { continue };
            let (x, yy) = self.to_screen((run[0].0 + run[1].0) / 2.0, run[0].1);
            let _ = ctx.fill_text(&format!("P{}", p.name), x, yy - 9.0);
        }
        for (k, p) in y.portals.iter().enumerate() {
            let (x, yy) = self.portal_xy(k);
            // North portals sit at the left end, south at the right; the
            // label goes outside the yard.
            let u = self.schematic.node[p.node].expect("portal node is drawn").0;
            let dx = if u < (self.all.0 + self.all.1) / 2.0 { -18.0 } else { 18.0 };
            let _ = ctx.fill_text(&p.name, x + dx, yy);
        }
        // Signal names, beyond the head on the side away from the track.
        ctx.set_font("10px ui-monospace, \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace");
        for (k, s) in y.signals.iter().enumerate() {
            let ((_, _), (du, dv)) = self.sig_path[k][self.sig_pick[k]];
            let (x, yy) = self.head_screen(k);
            let _ = ctx.fill_text(&s.name, x - dv * 18.0, yy - du * 18.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel lists every constant, with the value and tag the source has.
    #[test]
    fn the_panel_lists_every_constant_with_its_tag() {
        let rows = constant_rows();
        let n = include_str!("../../sim/src/consts.rs").lines().filter(|l| l.trim_start().starts_with("pub const")).count();
        assert_eq!(rows.len(), n);
        let f = |name: &str| rows.iter().find(|r| r.starts_with(&format!("{name}\t"))).map(|r| r.split('\t').map(str::to_string).collect::<Vec<_>>()).unwrap();
        assert_eq!(f("OVERLAP_M")[1..3], ["120.0", "solid"]);
        assert_eq!(f("SHIFT_START_S")[1..3], ["6 * 3600 + 1800", "assumed"]);
        assert!(f("TIME_SCALE")[3].starts_with("game seconds per real second"));
        assert!(rows.iter().all(|r| r.split('\t').count() == 4 && ["solid", "assumed"].contains(&r.split('\t').nth(2).unwrap())));
    }

    #[test]
    fn rubber_band_gives_way_and_never_reaches_the_view_size() {
        assert_eq!(band(5.0, 0.0, 10.0, 100.0), 5.0);
        let past = band(-50.0, 0.0, 10.0, 100.0);
        assert!(past < 0.0 && past > -50.0 * RUBBER - 1e-9, "past the edge it moves less than the pointer: {past}");
        assert!(band(-1e9, 0.0, 10.0, 100.0) > -100.0);
    }
}
