"""B2 verification: the skeleton page, driven with real mouse input.

Checks the B2 "done" line of PLAN-cabin.md: zero console errors on a fresh
and a seeded profile, 1920 px, no network request. Plus what the skeleton
must do: draw the yard, pan 1:1, rubber-band past an edge and spring back,
zoom about the cursor, run the sim clock. Since PLAN-look.md (2026-11-19)
the page is dark only, on a light system too, and the theme checks became
that check; a fresh profile writes nothing, a migrated one is written back.

Read-only hooks on CABIN.app give screen positions; every action is a real
click, drag or wheel. Usage: python3 tools/verify_b2.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import json
import pathlib
import sys

from playwright.sync_api import sync_playwright

PAGE_FILE = pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html"
PAGE = PAGE_FILE.as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
OUT.mkdir(parents=True, exist_ok=True)
VIEW = {"width": 1920, "height": 1080}
results: list[tuple[bool, str]] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg, n: int = 2) -> None:
    pg.evaluate(f"new Promise(r => {{ let k = {n}; const f = () => --k ? requestAnimationFrame(f) : r(); requestAnimationFrame(f); }})")


def open_page(b, scheme="dark", reduce=False, seed: str | None = None):
    """A fresh context is a fresh profile. `seed` is written to cabin.v1
    before the page's first script runs."""
    ctx = b.new_context(viewport=VIEW, color_scheme=scheme, reduced_motion="reduce" if reduce else "no-preference")
    if seed is not None:
        ctx.add_init_script(f"if (!sessionStorage.getItem('seeded')) {{ localStorage.setItem('cabin.v1', {json.dumps(seed)}); sessionStorage.setItem('seeded', '1'); }}")
    pg = ctx.new_page()
    errs: list[str] = []
    reqs: list[str] = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("request", lambda r: reqs.append(r.url))
    # NDLS by name: a bare URL opens the first ladder level since B10.
    pg.goto(PAGE + "?station=ndls")
    pg.wait_for_timeout(800)
    return ctx, pg, errs, reqs


def canvas_box(pg):
    b = pg.locator("#map").bounding_box()
    return b["x"], b["y"], b["width"], b["height"]


def inked(pg) -> float:
    """Share of canvas pixels that differ from the background colour."""
    return pg.evaluate("""(() => {
      const c = document.getElementById('map'), g = c.getContext('2d');
      const d = g.getImageData(0, 0, c.width, c.height).data;
      const r = d[0], gg = d[1], b = d[2]; let n = 0;
      for (let i = 0; i < d.length; i += 16) if (Math.abs(d[i]-r) + Math.abs(d[i+1]-gg) + Math.abs(d[i+2]-b) > 30) n++;
      return n / (d.length / 16);
    })()""")


def corner_rgb(pg):
    return pg.evaluate("Array.from(document.getElementById('map').getContext('2d').getImageData(2, 2, 1, 1).data.slice(0, 3))")


html = PAGE_FILE.read_text()
check("alert(" not in html and "confirm(" not in html, "built page holds no alert() or confirm()")

with sync_playwright() as pw:
    b = pw.chromium.launch()

    # ---- Fresh profile, dark ------------------------------------------------
    ctx, pg, errs, reqs = open_page(b, "dark")
    ox, oy, cw, ch = canvas_box(pg)
    check(cw >= 0.6 * VIEW["width"], f"panel takes two thirds at 1920 px: {cw:.0f} px wide")
    check(corner_rgb(pg) == [0, 0, 0] and not pg.locator("#theme").count(), "the panel is black and has no theme button")
    check(not pg.is_visible("#banner"), "no banner on a fresh profile")
    attr = pg.text_content("#attr")
    check("2026-11-15" in attr and "not to scale" in attr, f"attribution names the extract date and the stretch: {attr!r}")
    check(app(pg, "a.platform_count()") == 16 and app(pg, "a.edge_count()") == 323, "yard has 16 platforms and 323 edges")
    on = [app(pg, f"a.platform_screen({i})") for i in range(16)]
    check(all(0 <= x <= cw and 0 <= y <= ch for x, y in on), "every platform is on screen at the default view")
    share = inked(pg)
    check(share > 0.01, f"the yard is drawn: {share:.1%} of canvas pixels inked")
    pg.screenshot(path=str(OUT / "b2_dark_1920.png"))

    # The sim runs at TIME_SCALE; the clock follows it. (Until B6 this
    # checked the skeleton train, which B6 removed.)
    g0, t0 = app(pg, "a.game_s()"), pg.text_content("#clock")
    pg.wait_for_timeout(1200)
    g1 = app(pg, "a.game_s()")
    k = app(pg, "a.time_scale()")
    check(0.6 * k * 1.2 < g1 - g0 < 1.4 * k * 1.2, f"game time runs at about x{k}: {g1 - g0:.1f} game s in 1.2 real s")
    pg.wait_for_timeout(1000)
    check(pg.text_content("#clock") != t0, f"clock advances: {t0} -> {pg.text_content('#clock')}")

    # Wheel zoom about the cursor: a platform's offset from the cursor
    # scales by the zoom ratio.
    ax, ay = app(pg, "a.platform_screen(0)")
    mx, my = round(ax) + 37, round(ay) - 23
    z0 = app(pg, "a.zoom()")
    pg.mouse.move(ox + mx, oy + my)
    for _ in range(6):
        pg.mouse.wheel(0, -100)
        pg.wait_for_timeout(30)
    pg.wait_for_timeout(900)
    z1 = app(pg, "a.zoom()")
    bx, by = app(pg, "a.platform_screen(0)")
    k = z1 / z0
    ex, ey = mx + (ax - mx) * k, my + (ay - my) * k
    check(z1 > 2.5 * z0, f"wheel zooms in: {z0:.3f} -> {z1:.3f} px/m")
    check(abs(bx - ex) < 0.1 and abs(by - ey) < 0.1, f"zoom pivots on the cursor: platform at {bx:.2f},{by:.2f}, expected {ex:.2f},{ey:.2f}")

    # Drag pans 1:1 inside the yard.
    pg.mouse.move(ox + 600, oy + 500); pg.mouse.down(); pg.mouse.move(ox + 700, oy + 560, steps=6); pg.mouse.up()
    frames(pg)
    cx, cy = app(pg, "a.platform_screen(0)")
    check(abs((cx - bx) - 100) < 0.5 and abs((cy - by) - 60) < 0.5, f"drag pans 1:1: moved {cx - bx:.1f},{cy - by:.1f} for 100,60")

    # Past the left edge: the map gives way less than the pointer, then
    # springs back inside on release.
    u0, u1, v0, v1 = app(pg, "a.panel_bounds()")
    # Since B6 the default view frames the platforms, farther from the left
    # edge: plain drags bring the centre within 300 px of it first.
    for _ in range(10):
        if (app(pg, "a.centre()")[0] - u0) * app(pg, "a.zoom()") < 300:
            break
        pg.mouse.move(ox + 100, oy + 500); pg.mouse.down(); pg.mouse.move(ox + 1100, oy + 500, steps=10); pg.mouse.up()
        pg.wait_for_timeout(100)
    pg.mouse.move(ox + 100, oy + 500); pg.mouse.down()
    for step in range(1, 31):
        pg.mouse.move(ox + 100 + step * 100, oy + 500)
    frames(pg)
    free = app(pg, "a.centre()")[0]
    pulled = 3000 / app(pg, "a.zoom()")
    check(free < u0 and (u0 - free) < 0.55 * pulled, f"past the edge it rubber-bands: centre {u0 - free:.0f} m past, pointer asked {pulled:.0f} m")
    pg.mouse.up()
    pg.wait_for_timeout(1200)
    back = app(pg, "a.centre()")
    check(u0 <= back[0] <= u1 and v0 <= back[1] <= v1, f"on release it springs back inside: centre u {back[0]:.1f} (edge {u0:.1f})")
    check(not errs, f"no console errors, fresh profile: {errs}")

    check(pg.evaluate("localStorage.getItem('cabin.v1')") is None, "a fresh profile writes nothing until there is something to save")
    pg.keyboard.press("t")
    frames(pg)
    check(corner_rgb(pg) == [0, 0, 0], "T no longer switches the theme")
    check(all(u.startswith("file://") for u in reqs), f"no network request: {len(reqs)} requests, all file://")
    ctx.close()

    # A light system gets the same dark page.
    ctx, pg, errs, reqs = open_page(b, "light")
    check(corner_rgb(pg) == [0, 0, 0] and not errs, f"a light system still gets the dark panel {errs}")
    ctx.close()

    # ---- Seeded profiles ------------------------------------------------------
    # A v1 store: its delay best is a shift total, kept aside by the 1-to-2
    # migration and not shown as a per-train best.
    v1 = {"v": 1, "settings": {"theme": "light"}, "bests": {"light": {"handled": 3, "delay_min": 624.8, "put_backs": 1}}}
    ctx, pg, errs, reqs = open_page(b, "dark", seed=json.dumps(v1))
    check(corner_rgb(pg) == [0, 0, 0], "a profile saved with the light theme opens dark")
    check(not pg.is_visible("#banner") and not errs, f"seeded profile: no banner, no console errors {errs}")
    check(all(u.startswith("file://") for u in reqs), "seeded profile: no network request")
    mig = json.loads(pg.evaluate("localStorage.getItem('cabin.v1')"))
    check(mig.get("v") == 3 and mig.get("progress") == {} and mig.get("bests") == {"light": {"handled": 3, "put_backs": 1}} and mig.get("v1_delay_totals") == {"light": 624.8},
          f"v1 store migrates through v2 to v3 on load, is written back, and keeps the old delay total aside: {mig}")
    check(mig.get("settings") == {"theme": "light"}, f"the old theme setting is kept, unread: {mig.get('settings')}")
    ctx.close()

    for bad, why in (("not json", "not JSON"), (json.dumps({"v": 99}), "a future version")):
        ctx, pg, errs, reqs = open_page(b, "dark", seed=bad)
        text = pg.text_content("#banner") if pg.is_visible("#banner") else ""
        check("could not be read" in text, f"store holding {why}: banner says so: {text!r}")
        g0 = app(pg, "a.game_s()")
        pg.wait_for_timeout(500)
        check(app(pg, "a.game_s()") > g0, f"store holding {why}: the page still runs")
        # Guard 3 (B8): the unreadable copy moves to cabin.v1.corrupt and the
        # page saves again. Until B8 the copy was left in place, unsaved.
        check(pg.evaluate("localStorage.getItem('cabin.v1.corrupt')") == bad and json.loads(pg.evaluate("localStorage.getItem('cabin.v1')"))["v"] == 3,
              f"store holding {why}: the copy is kept under cabin.v1.corrupt, and a readable store is written back at once")
        check(not errs, f"store holding {why}: no console errors {errs}")
        ctx.close()

    # ---- Reduced motion: zoom jumps -------------------------------------------
    ctx, pg, errs, reqs = open_page(b, "dark", reduce=True)
    ox, oy, cw, ch = canvas_box(pg)
    z0 = app(pg, "a.zoom()")
    pg.mouse.move(ox + cw / 2, oy + ch / 2)
    pg.mouse.wheel(0, -100)
    frames(pg, 2)
    z1 = app(pg, "a.zoom()")
    check(abs(z1 / z0 - 1.2214) < 0.001, f"reduced motion: one notch zooms 1.22x on the next frame: {z1 / z0:.4f}")
    check(not errs, f"reduced motion: no console errors {errs}")
    ctx.close()
    b.close()

failed = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} passed")
sys.exit(1 if failed else 0)
