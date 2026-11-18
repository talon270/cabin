"""S2 and S3 verification: routes set by real gestures draw as unbroken
lines on the schematic, and each indication reads its colour from canvas
pixels, in both themes, at 1920 and 1500 px.

For each theme and width it opens practice on seed 1, then for each Starter
head on the default view: a mouse click on the head chooses it, and a click
on its first exit button that sets a route sets it (portal exits are not on
the canvas, so their buttons are the control, as in verify_b6.py). After
three routes, it reads each route's drawn edges from the read-only hook
`CABIN.app.route_screen(id)` and samples the canvas every 1 px along them:
a sample passes if a pixel within 1 px has the route colour (`--route`,
white on dark, black on light). Any run of more than 2 px of failed samples
is a gap. Samples off the canvas are skipped. A route not set must fail the
same sampler. It saves a screenshot of each view.

S3: point lie, from `route_screen`'s sibling `lie_screen(4)`: 4 px from each
switch on screen, the pixel on the leg not lying is nearer `--bg` than
`--trk`, and a pixel within 1 px on the leg lying is nearer `--trk` (the control:
siding legs are 1 px wide, so one pixel may be half lit). Points within 30 px of a signal head are skipped:
the head or its name covers them. Points whose two legs are drawn on one
line (the samples under 2 px apart) are skipped and listed: a known S1
layout defect, 13 of 75 drawn points, in which no lie can show. Occupied track, at 1920 px in each theme: Accept the
first offer and route it in by clicks, as verify_b6.py does, run the train
onto the view, and read `--occ` at the middle of each occupied edge from
`occupied_screen()`.

Usage: python3 tools/verify_b9.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import pathlib
import sys

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
OUT.mkdir(parents=True, exist_ok=True)
GAP_PX = 2
results: list[tuple[bool, str]] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg) -> None:
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")


def set_three_routes(pg) -> list[tuple[str, str, int]]:
    """Up to three routes, each from a different Starter head on screen."""
    box = pg.locator("#map").bounding_box()
    names = app(pg, "a.signal_names()")
    xy = app(pg, "a.signal_screens()")
    picked = []
    for i, name in enumerate(names):
        x, y = xy[2 * i], xy[2 * i + 1]
        if not name.startswith("ST") or not (2 < x < box["width"] - 2 and 2 < y < box["height"] - 2):
            continue
        pg.mouse.click(box["x"] + x, box["y"] + y)
        frames(pg)
        if app(pg, "a.selected()") != name:
            continue
        for k, row in enumerate(app(pg, "a.exits()")):
            rid, exit_name, _ = row.split("\t")
            before = len(app(pg, "a.live_routes()"))
            pg.locator("#exits button").nth(k).click()
            frames(pg)
            if len(app(pg, "a.live_routes()")) > before:
                picked.append((name, exit_name, int(rid)))
                break
            if app(pg, "a.selected()") != name:
                pg.mouse.click(box["x"] + x, box["y"] + y)
                frames(pg)
        if app(pg, "a.selected()"):
            pg.keyboard.press("Escape")
            frames(pg)
        if len(picked) == 3:
            break
    return picked


PIX = """
      const canvas = document.getElementById('map');
      const rect = canvas.getBoundingClientRect();
      const k = canvas.width / rect.width;
      const {data, width, height} = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height);
      // A theme gives hex; paint it once to read it back as RGB.
      const rgb = name => { const p = document.createElement('canvas').getContext('2d');
        p.fillStyle = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
        p.fillRect(0, 0, 1, 1); return Array.from(p.getImageData(0, 0, 1, 1).data.slice(0, 3)); };
      const near = (x, y, c, r) => {
        for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) {
          const xx = Math.round(x * k) + dx, yy = Math.round(y * k) + dy;
          if (xx < 0 || yy < 0 || xx >= width || yy >= height) continue;
          const i = (yy * width + xx) * 4;
          if (Math.abs(data[i] - c[0]) <= 8 && Math.abs(data[i+1] - c[1]) <= 8 && Math.abs(data[i+2] - c[2]) <= 8) return true;
        }
        return false;
      };
      const onView = (x, y, m) => x > m && y > m && x < rect.width - m && y < rect.height - m;
"""


def lie(pg) -> dict:
    """At each drawn point on screen, 4 px from the switch: the leg not
    lying reads background, the leg lying does not."""
    return pg.evaluate("""() => {""" + PIX + """
      const bg = rgb('--bg'), trk = rgb('--trk');
      // A gap pixel may carry anti-aliasing from the leg beside it: it is a
      // gap if it is nearer the background than the track colour.
      const at = (x, y) => { const i = (Math.round(y * k) * width + Math.round(x * k)) * 4; return [data[i], data[i+1], data[i+2]]; };
      const d = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
      const q = Array.from(CABIN.app.lie_screen(4));
      const heads = Array.from(CABIN.app.signal_screens());
      let gap = 0, solid = 0, n = 0, skipped = 0; const bad = [], overlaid = [];
      for (let i = 0; i < q.length; i += 5) {
        const [pt, ox, oy, lx, ly] = q.slice(i, i + 5);
        if (!onView(ox, oy, 4) || !onView(lx, ly, 4)) continue;
        if (Math.hypot(ox - lx, oy - ly) < 2) { overlaid.push(pt); continue; }
        let covered = false;
        for (let j = 0; j < heads.length; j += 2) if (Math.hypot(heads[j] - ox, heads[j+1] - oy) < 30 || Math.hypot(heads[j] - lx, heads[j+1] - ly) < 30) covered = true;
        if (covered) { skipped++; continue; }
        n++;
        const g = d(at(ox, oy), bg) < d(at(ox, oy), trk), s = [-1, 0, 1].some(dy => [-1, 0, 1].some(dx => { const c = at(lx + dx / k, ly + dy / k); return d(c, bg) > d(c, trk); }));
        if (g) gap++; if (s) solid++;
        if ((!g || !s) && bad.length < 3) bad.push([pt, Math.round(ox), Math.round(oy), g, s, at(ox, oy), at(lx, ly)]);
      }
      return {n, gap, solid, skipped, overlaid, bad};
    }""")


def occupied(pg) -> dict:
    """The middle of each occupied edge on screen reads `--occ`."""
    return pg.evaluate("""() => {""" + PIX + """
      const occ = rgb('--occ');
      const m = Array.from(CABIN.app.occupied_screen());
      let n = 0, red = 0;
      for (let i = 0; i < m.length; i += 2) {
        if (!onView(m[i], m[i+1], 2)) continue;
        n++; if (near(m[i], m[i+1], occ, 1)) red++;
      }
      return {edges: m.length / 2, n, red, occ};
    }""")


def run_train_in(pg, tag: str) -> None:
    """verify_b6.py's flow: Accept the first offer, wait for it at its Home,
    choose the Home by the rack's Route button, set the first platform exit
    by its button, and run until the train is on the view."""
    rack = lambda: [r.split("\t") for r in app(pg, "a.rack()")]
    offer = next((r for r in rack() if r[4] == "accept"), None)
    check(offer is not None, f"{tag}: a train is offered")
    if offer is None:
        return
    pg.locator(f'.bay .row[data-key="{offer[1]}"] button').click()
    frames(pg)
    tid = next(r[1] for r in rack() if r[1].startswith("t"))
    row = None
    for _ in range(60):
        row = next((r for r in rack() if r[1] == tid), None)
        if row and "needs a route" in row[3]:
            break
        app(pg, "a.skip(10)")
        frames(pg)
    pg.locator(f'.bay .row[data-key="{tid}"] button').click()
    frames(pg)
    exits = [r.split("\t") for r in app(pg, "a.exits()")]
    k = next((i for i, r in enumerate(exits) if r[1].startswith("P")), None)
    check(k is not None, f"{tag}: the Home offers a platform exit")
    if k is None:
        return
    pg.locator("#exits button").nth(k).click()
    frames(pg)
    for _ in range(120):
        if occupied(pg)["n"] >= 2:
            break
        app(pg, "a.skip(5)")
        frames(pg)


def gaps(pg, rid: int) -> dict:
    """Sample the canvas along route `rid`'s drawn edges."""
    return pg.evaluate("""({rid, gapPx}) => {
      const canvas = document.getElementById('map');
      const rect = canvas.getBoundingClientRect();
      const k = canvas.width / rect.width;
      const {data, width, height} = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height);
      // The theme gives hex (#fff); paint it once to read it back as RGB.
      const probe = document.createElement('canvas').getContext('2d');
      probe.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--route').trim();
      probe.fillRect(0, 0, 1, 1);
      const fg = probe.getImageData(0, 0, 1, 1).data;
      const lit = (x, y) => {
        for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
          const xx = Math.round(x * k) + dx, yy = Math.round(y * k) + dy;
          if (xx < 0 || yy < 0 || xx >= width || yy >= height) continue;
          const i = (yy * width + xx) * 4;
          if (Math.abs(data[i] - fg[0]) <= 8 && Math.abs(data[i+1] - fg[1]) <= 8 && Math.abs(data[i+2] - fg[2]) <= 8) return true;
        }
        return false;
      };
      const pts = Array.from(CABIN.app.route_screen(rid));
      let samples = 0, worst = 0, run = 0, at = null;
      for (let i = 0; i + 3 < pts.length; i += 2) {
        const [x0, y0, x1, y1] = [pts[i], pts[i+1], pts[i+2], pts[i+3]];
        if ([x0, y0, x1, y1].some(Number.isNaN)) { run = 0; continue; }
        const n = Math.max(1, Math.ceil(Math.hypot(x1 - x0, y1 - y0)));
        for (let s = 0; s <= n; s++) {
          const x = x0 + (x1 - x0) * s / n, y = y0 + (y1 - y0) * s / n;
          if (x < 1 || y < 1 || x > rect.width - 1 || y > rect.height - 1) { run = 0; continue; }
          samples++;
          if (lit(x, y)) { run = 0; } else { run++; if (run > worst) { worst = run; at = [Math.round(x), Math.round(y)]; } }
        }
      }
      return {ok: samples > 0 && worst <= gapPx, samples, worstGapPx: worst, at};
    }""", {"rid": rid, "gapPx": GAP_PX})


with sync_playwright() as pw:
    browser = pw.chromium.launch()
    for width, height in [(1920, 1080), (1500, 900)]:
        for theme in ["dark", "light"]:
            ctx = browser.new_context(viewport={"width": width, "height": height}, color_scheme=theme)
            pg = ctx.new_page()
            errors: list[str] = []
            pg.on("pageerror", lambda e: errors.append(str(e)))
            pg.goto(PAGE + "?mode=practice&seed=1")
            pg.wait_for_timeout(600)
            tag = f"{theme} {width}x{height}"
            check(pg.evaluate("document.documentElement.dataset.theme") == theme and not errors, f"{tag}: page starts with no errors")
            routes = set_three_routes(pg)
            check(len(routes) == 3, f"{tag}: three routes set by clicks ({[(e, x) for e, x, _ in routes]})")
            frames(pg)
            for entry, exit_name, rid in routes:
                g = gaps(pg, rid)
                check(g["ok"], f"{tag}: {entry} to {exit_name} drawn unbroken ({g})")
            # Control: a route not set must read as gapped, or the sampler
            # proves nothing (every track would pass).
            set_ids = {rid for _, _, rid in routes}
            control = next((g for g in (gaps(pg, r) for r in range(60) if r not in set_ids) if g["samples"] > 100), None)
            check(control is not None and not control["ok"], f"{tag}: an unset route reads as gapped ({control})")
            route_rgb = pg.evaluate("() => {" + PIX + " return rgb('--route'); }")
            check(route_rgb == ([255] * 3 if theme == "dark" else [0] * 3), f"{tag}: the route colour is {'white' if theme == 'dark' else 'black'} ({route_rgb})")
            lz = lie(pg)
            check(lz["n"] >= 10 and lz["gap"] == lz["n"] and lz["solid"] == lz["n"], f"{tag}: point lie: the leg not lying is broken at the switch, the leg lying is not ({lz})")
            check(not errors, f"{tag}: no page errors ({errors[:2]})")
            pg.screenshot(path=str(OUT / f"b9-{theme}-{width}.png"))
            ctx.close()
        if width != 1920:
            continue
        for theme in ["dark", "light"]:
            ctx = browser.new_context(viewport={"width": width, "height": height}, color_scheme=theme)
            pg = ctx.new_page()
            errors = []
            pg.on("pageerror", lambda e: errors.append(str(e)))
            pg.goto(PAGE + "?seed=1")
            pg.wait_for_timeout(600)
            tag = f"{theme} {width}x{height} occupied"
            run_train_in(pg, tag)
            o = occupied(pg)
            check(o["n"] >= 2 and o["red"] == o["n"], f"{tag}: each occupied edge on the view reads --occ at its middle ({o})")
            check(not errors, f"{tag}: no page errors ({errors[:2]})")
            pg.screenshot(path=str(OUT / f"b9-{theme}-occupied.png"))
            ctx.close()
    browser.close()

failed = [w for ok, w in results if not ok]
print(f"RESULT {len(results) - len(failed)}/{len(results)} passed", flush=True)
sys.exit(1 if failed else 0)
