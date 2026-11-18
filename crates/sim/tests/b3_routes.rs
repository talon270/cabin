//! B3: the signals generated for NDLS and the routes they give.
//!
//! The legality checks re-read the yard's point and crossing records
//! directly; they do not call `Graph::next`, which the route search uses.

use cabin_sim::consts::{HOME_SETBACK_M, OVERLAP_M, STARTER_SETBACK_M};
use cabin_sim::route::{build_routes, Exit, Res, Route};
use cabin_sim::yard::{Kind, SignalKind, Yard};

fn ndls() -> Yard {
    Yard::parse(cabin_sim::NDLS).expect("data/ndls.yard parses")
}

/// The shared node of two consecutive edges, and the direction the second
/// is run in.
fn meet(y: &Yard, x: usize, xf: bool, n: usize) -> (usize, bool) {
    let head = if xf { y.edges[x].b } else { y.edges[x].a };
    let e = &y.edges[n];
    assert!(e.a == head || e.b == head, "edge {n} does not start where {x} ends");
    (head, e.a == head)
}

/// (edge, forward) for every edge of the route's path.
fn running(y: &Yard, r: &Route) -> Vec<(usize, bool)> {
    let mut out = vec![(r.path[0], y.signals[r.entry].forward)];
    for &n in &r.path[1..] {
        let &(x, xf) = out.last().unwrap();
        let (_, f) = meet(y, x, xf, n);
        out.push((n, f));
    }
    out
}

#[test]
fn every_portal_has_one_home_and_every_signal_stands_on_running_track() {
    let y = ndls();
    let homes: Vec<&str> = y.signals.iter().filter(|s| s.kind == SignalKind::Home).map(|s| s.name.as_str()).collect();
    assert_eq!(homes.len(), y.portals.len(), "{homes:?}");
    for s in &y.signals {
        assert_eq!(y.edges[s.edge].kind, Kind::Running, "{} stands on {:?} track", s.name, y.edges[s.edge].kind);
        assert_eq!(s.kind == SignalKind::Starter, s.platform.is_some(), "{}: only a Starter has a platform", s.name);
    }
    for (k, p) in y.platforms.iter().enumerate() {
        let n = y.signals.iter().filter(|s| s.platform == Some(k)).count();
        assert!((1..=2).contains(&n), "platform {} has {n} Starters", p.name);
    }
}

#[test]
fn a_signal_stands_its_setback_before_the_junction_it_protects() {
    let y = ndls();
    for s in &y.signals {
        let len = y.edges[s.edge].len;
        let to_head = if s.forward { len - s.offset } else { s.offset };
        let want = match s.kind {
            SignalKind::Home => HOME_SETBACK_M,
            SignalKind::Starter => STARTER_SETBACK_M,
        };
        // Clamped only where the edge is shorter than the setback.
        assert!((to_head - want.min(len)).abs() < 0.05, "{}: {to_head:.1} m before its junction, want {want}", s.name);
    }
}

#[test]
fn every_route_obeys_points_and_crossings() {
    let y = ndls();
    for r in build_routes(&y) {
        let run = running(&y, &r);
        for w in run.windows(2) {
            let ((x, xf), (n, _)) = (w[0], w[1]);
            let node = if xf { y.edges[x].b } else { y.edges[x].a };
            if let Some(p) = y.points.iter().find(|p| p.node == node) {
                let ok = (x == p.stem && (n == p.normal || n == p.reverse)) || (n == p.stem && (x == p.normal || x == p.reverse));
                assert!(ok, "route {}: {x} -> {n} turns through point {} leg to leg", r.id, y.nodes[node].osm);
            }
            if let Some(c) = y.crossings.iter().find(|c| c.node == node) {
                assert!(c.pairs.iter().any(|&(a, b)| (a, b) == (x, n) || (b, a) == (x, n)), "route {}: {x} -> {n} turns at a diamond", r.id);
            }
            assert_eq!(y.edges[n].kind, Kind::Running, "route {} runs over {:?} edge {n}", r.id, y.edges[n].kind);
        }
    }
}

#[test]
fn no_route_passes_a_signal_for_its_own_direction() {
    let y = ndls();
    let along = |e: usize, f: bool, off: f64| if f { off } else { y.edges[e].len - off };
    for r in build_routes(&y) {
        let run = running(&y, &r);
        let entry = &y.signals[r.entry];
        let last = run.len() - 1;
        for (i, &(e, f)) in run.iter().enumerate() {
            for (k, s) in y.signals.iter().enumerate() {
                if s.edge != e || s.forward != f || k == r.entry || matches!(r.exit, Exit::Berth { starter, .. } if starter == k) {
                    continue;
                }
                let at = along(e, f, s.offset);
                let after_entry = i > 0 || at > along(e, f, entry.offset);
                let before_exit = i < last
                    || match r.exit {
                        Exit::Berth { starter, .. } => at < along(e, f, y.signals[starter].offset),
                        Exit::Portal(_) => true,
                    };
                assert!(!(after_entry && before_exit), "route {} ({} on) passes {}", r.id, entry.name, s.name);
            }
        }
    }
}

#[test]
fn holds_cover_path_overlap_and_points_and_the_overlap_is_long_enough() {
    let y = ndls();
    let g = y.graph();
    for r in build_routes(&y) {
        for &e in r.path.iter().chain(&r.overlap) {
            assert!(r.holds.contains(&Res::Edge(e)), "route {}: edge {e} not held", r.id);
        }
        for &(p, _) in &r.points {
            assert!(r.holds.contains(&Res::Point(p)), "route {}: point {p} not held", r.id);
        }
        if let Exit::Berth { starter, .. } = r.exit {
            let x = &y.signals[starter];
            let first = if x.forward { y.edges[x.edge].len - x.offset } else { x.offset };
            let total = first + r.overlap.iter().map(|&e| y.edges[e].len).sum::<f64>();
            // Short only where the line ends: a buffer stop or the box edge.
            let (mut e, mut f) = (x.edge, x.forward);
            for &n in &r.overlap {
                (_, f) = meet(&y, e, f, n);
                e = n;
            }
            let ends = g.next(&y, e, f).is_empty();
            assert!(total >= OVERLAP_M || ends, "route {}: overlap {total:.0} m beyond {}", r.id, x.name);
        }
    }
}

#[test]
fn no_route_leads_into_a_berth_the_train_cannot_leave() {
    let y = ndls();
    let routes = build_routes(&y);
    for r in &routes {
        if let Exit::Berth { starter, .. } = r.exit {
            assert!(routes.iter().any(|d| d.entry == starter), "route {} ends at {}, which sets no route", r.id, y.signals[starter].name);
        }
        assert_eq!(routes[r.id].id, r.id, "ids are indices");
    }
}

#[test]
fn routes_are_the_same_on_every_build() {
    let y = ndls();
    assert_eq!(build_routes(&y), build_routes(&y));
}

#[test]
fn every_platform_but_15_and_16_and_every_home_has_a_route() {
    let y = ndls();
    let routes = build_routes(&y);
    let mut unworked = Vec::new();
    for (k, p) in y.platforms.iter().enumerate() {
        let arrive = routes.iter().any(|r| matches!(r.exit, Exit::Berth { platform, .. } if platform == k));
        let leave = routes.iter().any(|r| y.signals[r.entry].platform == Some(k) && matches!(r.exit, Exit::Portal(_)));
        assert!(leave, "platform {}: no departure", p.name);
        if !arrive {
            unworked.push(p.name.as_str());
        }
    }
    // Platforms 15 and 16's only arrivals came from the south into ST15N
    // and ST16N, which set no route; with those dropped no train reaches
    // either (2026-11-17 yard). No train could ever leave them before.
    unworked.sort();
    assert_eq!(unworked, ["15", "16"], "platforms with no arrival");
    for (k, s) in y.signals.iter().enumerate() {
        if s.kind == SignalKind::Home {
            assert!(routes.iter().any(|r| r.entry == k), "{} sets no route", s.name);
        }
    }
}
