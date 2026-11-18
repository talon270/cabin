"""B8 verification: every edge made to fail, the "what is real" panel,
Guard 3, reduced motion, both themes, the colour rule, frame time at Peak.

Checks the B8 "done" line of PLAN-cabin.md: it prints what the page or the
tool shows for each row of the plan's edges table, and the median frame
time at Peak.

The tool drills never touch `data/` or `rails/`. `fetch-osm.sh` and
`cabin-osm` read `CABIN_DATA`, a copy of `data/` in the scratch folder;
`fetch-osm.sh` reads `OVERPASS_URL`, a canned answer on disk; the ledger
test reads `CABIN_LEDGER`, an edited copy of `rails/research.md`. Each drill
then compares the scratch file it guards with the copy taken before.

Page drills use real clicks and keys, with the `verify_b6.py` exception:
`CABIN.app.skip(s)` runs game time at once. Storage that is blocked, full or
unreadable and a wasm module that will not start are made by init scripts,
which is the only way to make a browser refuse them. Frame time wraps the
App's `frame` method to time it; nothing else is changed.

Usage: python3 tools/verify_b8.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import statistics
import subprocess
import sys

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
OUT.mkdir(parents=True, exist_ok=True)
VIEW = {"width": 1920, "height": 1080}
SHIFT_S = 90 * 60
ASPECTS = ["#e5484d", "#f5d90a", "#30a46c"]
ENV = {**os.environ, "CARGO_TARGET_DIR": os.environ.get("CARGO_TARGET_DIR", str(pathlib.Path.home() / ".cache/cabin-target"))}
results: list[tuple[bool, str]] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def quote(what: str, text: str) -> None:
    """Print what a tool or the page showed, for the report."""
    print(f"  {what}: " + " | ".join(l.strip() for l in text.strip().splitlines() if l.strip())[:400], flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg, n: int = 3) -> None:
    pg.evaluate(f"new Promise(r => {{ let k = {n}; const f = () => --k ? requestAnimationFrame(f) : r(); requestAnimationFrame(f); }})")


def text(pg, sel: str) -> str:
    frames(pg)
    return pg.locator(sel).inner_text()


def stored(pg, key: str):
    return pg.evaluate(f"localStorage.getItem({json.dumps(key)})")


def tab(ctx, query: str = "?mode=light&seed=1", pause: bool = True):
    pg = ctx.new_page()
    pg.errs, pg.reqs = [], []
    pg.on("pageerror", lambda e: pg.errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and pg.errs.append(m.text))
    pg.on("request", lambda r: pg.reqs.append(r.url))
    pg.goto(PAGE + query)
    pg.wait_for_timeout(600)
    if pause and pg.evaluate("!!(CABIN.app)"):
        pg.keyboard.press("Space")
    return pg


def clean(pg, what: str) -> None:
    check(not pg.errs, f"{what}: no console error {pg.errs[:3]}")
    check(all(r.startswith("file:") for r in pg.reqs), f"{what}: no network request")


def seeded(b, init: str, scheme: str = "dark", motion: str = "no-preference"):
    ctx = b.new_context(viewport=VIEW, color_scheme=scheme, reduced_motion=motion)
    if init:
        ctx.add_init_script(init)
    return ctx


def seed_once(values: dict[str, str]) -> str:
    """An init script that writes storage on the first load only."""
    sets = "".join(f"localStorage.setItem({json.dumps(k)}, {json.dumps(v)});" for k, v in values.items())
    return f"if (!sessionStorage.getItem('seeded')) {{ {sets} sessionStorage.setItem('seeded', '1'); }}"


# ---- Page edges --------------------------------------------------------------

def guard3(b) -> None:
    """A corrupt or future store is kept under cabin.v1.corrupt, a banner
    says so, defaults play, and new bests are saved."""
    for name, raw, why in [("not json", "not json", "JSON"), ('{"schema":99}', '{"schema":99}', "no version"),
                           ("future v99", '{"v":99,"settings":{"theme":"light"},"bests":{"light":{"handled":40}}}', "newer cabin (v99)")]:
        ctx = seeded(b, seed_once({"cabin.v1": raw}))
        pg = tab(ctx)
        banner = text(pg, "#banner")
        quote(f"store {name}, banner", banner)
        check(pg.locator("#banner").is_visible() and why in banner and "kept under cabin.v1.corrupt" in banner, f"store {name}: the banner names the cause and where the copy is")
        check(stored(pg, "cabin.v1.corrupt") == raw, f"store {name}: the old value is kept under cabin.v1.corrupt")
        check(text(pg, "#mode") == "light" and app(pg, "a.clock_s()") >= 6.5 * 3600, f"store {name}: the game plays on defaults")
        app(pg, f"a.skip({SHIFT_S + 1})")
        st = text(pg, "#msg")
        try:
            saved = json.loads(stored(pg, "cabin.v1") or "null")
        except ValueError:
            saved = None
        check(saved and saved.get("v") == 2 and "light" in saved.get("bests", {}) and "not saved" not in st, f"store {name}: the next best is saved: {saved and saved.get('bests')}")
        clean(pg, f"store {name}")
        ctx.close()

    # A different copy already kept: nothing is overwritten, nothing saved.
    ctx = seeded(b, seed_once({"cabin.v1": "not json", "cabin.v1.corrupt": "an older copy"}))
    pg = tab(ctx)
    banner = text(pg, "#banner")
    quote("store not json, cabin.v1.corrupt taken, banner", banner)
    pg.keyboard.press("t")
    app(pg, f"a.skip({SHIFT_S + 1})")
    st = text(pg, "#msg")
    check(stored(pg, "cabin.v1") == "not json" and stored(pg, "cabin.v1.corrupt") == "an older copy" and "not saved in this browser" in st,
          f"store not json, cabin.v1.corrupt taken: both kept, the result says bests are not saved")
    ctx.close()


def storage_blocked_or_full(b) -> None:
    ctx = seeded(b, "Object.defineProperty(window, 'localStorage', { get() { throw new DOMException('blocked by the test', 'SecurityError'); } });")
    pg = tab(ctx)
    banner = text(pg, "#banner")
    quote("storage blocked, banner", banner)
    check(banner.startswith("Bests are not saved in this browser."), "storage blocked: the banner says bests are not saved")
    c0 = app(pg, "a.clock_s()")
    pg.keyboard.press("Space")
    pg.wait_for_timeout(500)
    check(app(pg, "a.clock_s()") > c0, "storage blocked: the game plays")
    pg.keyboard.press("Space")
    app(pg, f"a.skip({SHIFT_S + 1})")
    st = text(pg, "#msg")
    quote("storage blocked, end of shift", st)
    check("Bests are not saved in this browser" in st, "storage blocked: the result line says so")
    ctx.close()

    ctx = seeded(b, "Storage.prototype.setItem = function () { throw new DOMException('full in the test', 'QuotaExceededError'); };")
    pg = tab(ctx)
    pg.keyboard.press("t")
    banner = text(pg, "#banner")
    quote("storage full, banner after a theme change", banner)
    check(banner.startswith("Bests are not saved in this browser.") and "full or blocked" in banner, "storage full: the banner says bests are not saved")
    app(pg, f"a.skip({SHIFT_S + 1})")
    check("Bests are not saved in this browser" in text(pg, "#msg"), "storage full: the result line says so")
    ctx.close()


def wasm_fails(b) -> None:
    ctx = seeded(b, "WebAssembly.Module = function () { throw new Error('wasm refused by the test'); };")
    pg = tab(ctx, pause=False)
    pg.wait_for_timeout(300)
    banner = text(pg, "#banner")
    quote("wasm refused, banner", banner)
    style = pg.evaluate("(() => { const s = getComputedStyle(document.getElementById('banner')); return [s.borderBottomColor, s.color]; })()")
    check(banner.startswith("The panel failed to start.") and "wasm refused by the test" in banner, "wasm fails: the banner gives the error text")
    check(pg.locator("header").is_visible() and style[0] == style[1], f"wasm fails: the header stays and the banner is ink-bordered, not red: {style}")
    pg.screenshot(path=str(OUT / "b8-wasm-failed.png"))
    ctx.close()


def second_tab(b) -> None:
    """Two real tabs on one store, each ending a Light shift: the store keeps
    the better value per field."""
    ctx = seeded(b, "")
    a, c = tab(ctx, "?mode=light&seed=1"), tab(ctx, "?mode=light&seed=2")
    app(c, f"a.skip({SHIFT_S + 1})")
    frames(c)
    first = json.loads(stored(c, "cabin.v1"))["bests"]["light"]
    app(a, f"a.skip({SHIFT_S + 1})")
    frames(a)
    both = json.loads(stored(a, "cabin.v1"))["bests"]["light"]
    da = float(text(a, "#delay"))
    quote("second tab", f"tab seed 2 saved {first}; tab seed 1 then saved, store holds {both}; seed 1 delay {da}")
    check(both["delay_per_train"] == min(first["delay_per_train"], da) and both["handled"] == 0 and both["put_backs"] == 0, "second tab: the store keeps the better delay of the two")
    ctx.close()


def bad_seed_and_real(b) -> None:
    for scheme in ["dark", "light"]:
        ctx = seeded(b, "", scheme)
        pg = tab(ctx, "?mode=normal&seed=abc")
        st = text(pg, "#msg")
        if scheme == "dark":
            quote("?seed=abc, status line", st)
            check(st.startswith('Seed "abc" is not a whole number') and text(pg, "#seed") in st, "?seed=abc: said, and the random seed played is printed")
        # Open the panel by its header button.
        pg.locator("#realopen").click()
        frames(pg)
        check(pg.locator("#real").is_visible(), f"{scheme}: What is real opens on a click")
        layers = [r.split("\t") for r in app(pg, "a.real()")]
        shown = pg.locator("#layers tr").count()
        consts = pg.locator("#consts tr").count()
        n_src = sum(1 for l in (ROOT / "crates/sim/src/consts.rs").read_text().splitlines() if l.lstrip().startswith("pub const"))
        check(shown == len(layers) == 12 and consts == n_src, f"{scheme}: 12 layers and all {n_src} constants listed ({shown}, {consts})")
        tags = pg.locator("#consts .tag").all_inner_texts()
        check(set(t.lower() for t in tags) == {"solid", "assumed"}, f"{scheme}: every constant is tagged solid or assumed ({sum(t.lower() == 'assumed' for t in tags)} assumed)")
        body = text(pg, "#real")
        osm = app(pg, "a.osm_base()")[:10]
        track = next(r for r in layers if r[0] == "Track")[2]
        traffic = next(r for r in layers if r[0] == "Traffic")[2]
        if scheme == "dark":
            quote("What is real, Track", track)
            quote("What is real, Traffic", traffic)
        check(f"Extract of {osm}" in track and "read 2026-11-15" in traffic and "About 275 trains" in traffic, f"{scheme}: the panel prints the extract date and the date the traffic was read")
        check("© OpenStreetMap contributors" in body and "ODbL" in body, f"{scheme}: the OpenStreetMap credit and licence")
        # Contrast inside the dialog, from computed colours.
        cols = pg.evaluate("""(() => {
          const g = s => getComputedStyle(document.querySelector(s));
          return [g('#real').backgroundColor, g('#real td').color, g('#real p').color, g('#real .label').color]; })()""")
        bg = rgb(cols[0])
        ratios = [round(contrast(rgb(c), bg), 2) for c in cols[1:]]
        check(min(ratios) >= 4.5, f"{scheme}: dialog text contrast {ratios} against {cols[0]}")
        pg.screenshot(path=str(OUT / f"b8-real-{scheme}.png"))
        pg.keyboard.press("Escape")
        frames(pg)
        check(not pg.locator("#real").is_visible(), f"{scheme}: one Esc closes it")
        # And from the command menu.
        pg.keyboard.press("Control+k")
        pg.keyboard.type("what is real")
        pg.keyboard.press("Enter")
        frames(pg)
        check(pg.locator("#real").is_visible(), f"{scheme}: the command menu opens it")
        pg.keyboard.press("Escape")
        clean(pg, f"panel, {scheme}")
        ctx.close()


def rgb(css: str) -> tuple[float, float, float]:
    return tuple(float(x) for x in re.findall(r"[\d.]+", css)[:3])


def contrast(a, b) -> float:
    def lum(c):
        ch = [v / 255 for v in c]
        ch = [v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4 for v in ch]
        return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2]
    la, lb = sorted([lum(a), lum(b)], reverse=True)
    return (la + 0.05) / (lb + 0.05)


def reduced_motion(b) -> None:
    for motion in ["reduce", "no-preference"]:
        ctx = seeded(b, "", motion=motion)
        pg = tab(ctx)
        box = pg.locator("#map").bounding_box()
        pg.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        z0 = app(pg, "a.zoom()")
        pg.mouse.wheel(0, -300)
        frames(pg, 2)
        z1 = app(pg, "a.zoom()")
        frames(pg, 60)
        z2 = app(pg, "a.zoom()")
        dur = pg.evaluate("getComputedStyle(document.querySelector('#pause')).transitionDuration")
        if motion == "reduce":
            check(z1 == z2 != z0 and set(dur.split(", ")) == {"0s"}, f"reduced motion: zoom jumps ({z0:.3f} -> {z1:.3f}, settled {z2:.3f}), button transition {dur}")
        else:
            check(z0 < z1 < z2, f"full motion: zoom springs ({z0:.3f} -> {z1:.3f} -> {z2:.3f})")
        ctx.close()


def frame_time(b) -> None:
    """Peak, seed 1: accept every offer for 45 game minutes so trains stand
    at the Homes, then time the App's frame over 300 real frames."""
    ctx = seeded(b, "")
    pg = tab(ctx, "?mode=peak&seed=1")
    for _ in range(27):
        for btn in pg.locator('.bay[data-bay="due"] .row button').all():
            btn.click()
        app(pg, "a.skip(100)")
        frames(pg)
    trains = app(pg, "a.train_count()")
    pg.evaluate("""(() => {
      const P = Object.getPrototypeOf(CABIN.app), f = P.frame;
      window.__ft = []; window.__gap = []; let last = 0;
      P.frame = function (...a) { const t = performance.now(); if (last) __gap.push(t - last); last = t;
        const r = f.apply(this, a); __ft.push(performance.now() - t); return r; };
    })()""")
    pg.keyboard.press("Space")
    pg.evaluate("new Promise(r => { const w = () => __ft.length >= 300 ? r() : setTimeout(w, 50); w(); })")
    ft, gap = pg.evaluate("[__ft, __gap]")
    ft.sort()
    med, p95 = statistics.median(ft), ft[int(len(ft) * 0.95)]
    print(f"  frame time at Peak, seed 1, {trains} trains in the yard, {len(ft)} frames: median {med:.2f} ms, 95th percentile {p95:.2f} ms, frame interval median {statistics.median(gap):.2f} ms (headless Chromium, software canvas)", flush=True)
    check(trains >= 8 and p95 < 16.7, f"frame time at Peak: 95th percentile {p95:.2f} ms is inside one 60 Hz frame, with {trains} trains")
    clean(pg, "Peak")
    ctx.close()


# ---- Static checks -----------------------------------------------------------

def colour_rule() -> None:
    """Red, yellow and green appear in the signal-head function only."""
    shell = (ROOT / "web/shell.html").read_text()
    page = (ROOT / "dist/index.html").read_text()
    page_text = re.sub(r'CABIN\.boot\("[^"]*"\)', "", page)
    hits = [h for h in ASPECTS if h in shell.lower() or h in page_text.lower()]
    # Named colours in a colour declaration; comments may say "red".
    named = re.findall(r"(?:color|background|fill|stroke|border[\w-]*)\s*:\s*[^;}]*\b(red|yellow|green|lime|orange)\b", shell + page_text, re.I)
    src = (ROOT / "crates/web/src/lib.rs").read_text().splitlines()
    start = next(i for i, l in enumerate(src) if "fn draw_heads" in l)
    end = next(i for i in range(start + 1, len(src)) if src[i].startswith("    fn ") or src[i].startswith("    pub fn "))
    outside = [i + 1 for i, l in enumerate(src) if any(h in l.lower() for h in ASPECTS) and not start <= i < end]
    check(not hits and not named and not outside, f"colour rule: aspect colours outside draw_heads: page {hits + named}, lib.rs lines {outside}")


# ---- Tool edges --------------------------------------------------------------

def run(cmd: list[str], **env: str) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=ROOT, env={**ENV, **env}, capture_output=True, text=True, timeout=900)


def fresh_data(name: str) -> pathlib.Path:
    d = OUT / f"data-{name}"
    shutil.rmtree(d, ignore_errors=True)
    shutil.copytree(ROOT / "data", d, ignore=shutil.ignore_patterns("*.backup-*"))
    return d


def overpass() -> None:
    for name, answer, want in [("empty", '{"version":0.6,"elements":[]}', "Overpass returned no ways for NDLS"),
                               ("remark", '{"elements":[{"type": "way","id":1}],"remark":"runtime error: Query timed out"}', "Overpass answered with an error remark")]:
        d = fresh_data(f"overpass-{name}")
        canned = OUT / f"overpass-{name}.json"
        canned.write_text(answer)
        before = (d / "ndls.osm.json").read_bytes()
        r = run(["sh", "fetch-osm.sh"], CABIN_DATA=str(d), OVERPASS_URL=canned.as_uri())
        quote(f"fetch-osm.sh, {name} answer (exit {r.returncode})", r.stderr + r.stdout)
        check(r.returncode == 1 and want in r.stderr and (d / "ndls.osm.json").read_bytes() == before and not (d / "ndls.osm.json.tmp").exists(),
              f"Overpass {name}: exit 1, the previous ndls.osm.json kept")


def osm_runs() -> None:
    run(["cargo", "build", "-q", "--release", "-p", "cabin-osm"])
    exe = [str(pathlib.Path(ENV["CARGO_TARGET_DIR"]) / "release" / "cabin-osm")]

    # An element the overrides name is gone from the map.
    d = fresh_data("osm-edit")
    osm = json.loads((d / "ndls.osm.json").read_text())
    osm["elements"] = [e for e in osm["elements"] if not (e["type"] == "way" and e["id"] == 770843509)]
    (d / "ndls.osm.json").write_text(json.dumps(osm))
    before = (d / "ndls.yard").read_bytes()
    r = run(exe, CABIN_DATA=str(d))
    quote(f"cabin-osm, way 770843509 deleted (exit {r.returncode})", r.stderr)
    check(r.returncode == 1 and "way 770843509" in r.stderr and (d / "ndls.yard").read_bytes() == before, "OSM edit: cabin-osm names the missing id, exits 1, the old ndls.yard stays")

    # A platform's track cut off from every other track: its nodes renamed.
    d = fresh_data("unreachable")
    yard = (d / "ndls.yard").read_text().splitlines()
    edges = [l.split() for l in yard if l.startswith("edge ")]
    plat = next(l.split() for l in yard if l.startswith("platform 6 "))
    way = int(edges[int(plat[2])][5])
    osm = json.loads((d / "ndls.osm.json").read_text())
    nodes = {e["id"]: e for e in osm["elements"] if e["type"] == "node"}
    w = next(e for e in osm["elements"] if e["type"] == "way" and e["id"] == way)
    fresh = []
    for k, n in enumerate(w["nodes"]):
        copy = {**nodes[n], "id": 9_000_000_000_000 + k}
        copy.pop("tags", None)
        fresh.append(copy)
    w["nodes"] = [c["id"] for c in fresh]
    osm["elements"] += fresh
    (d / "ndls.osm.json").write_text(json.dumps(osm))
    before = (d / "ndls.yard").read_bytes()
    r = run(exe, CABIN_DATA=str(d))
    tail = "\n".join(l for l in r.stdout.splitlines() if l.startswith(("GATE", "  platform", "REFUSED")))
    quote(f"cabin-osm, platform 6's track (way {way}) cut off (exit {r.returncode})", tail + r.stderr)
    check(r.returncode == 1 and "platform 6 (way 34940731) cannot be reached from any portal" in r.stdout and (d / "ndls.yard").read_bytes() == before, "extract quality: the gate names the platform, exits 1, the old ndls.yard stays")


def ledger() -> None:
    src = ROOT.parents[1] / "rails" / "research.md"
    text_ = src.read_text()
    row = next(l for l in text_.splitlines() if l.startswith("| C3-143 |"))
    edited = OUT / "research-edited.md"
    edited.write_text(text_.replace(row, row.replace("120 m with multiple-aspect", "100 m with multiple-aspect")))
    r = run(["cargo", "test", "-q", "--release", "-p", "cabin-sim", "--test", "ledger"], CABIN_LEDGER=str(edited))
    msg = next((l for l in (r.stdout + r.stderr).splitlines() if "cites" in l), "")
    quote(f"cargo test, C3-143 edited (exit {r.returncode})", msg)
    check(r.returncode != 0 and "C3-143" in msg, "ledger row edited: cargo test fails and names the row")
    r = run(["cargo", "test", "-q", "--release", "-p", "cabin-sim", "--test", "ledger"], CABIN_LEDGER=str(OUT / "no-such-research.md"))
    msg = next((l for l in (r.stdout + r.stderr).splitlines() if "no-such-research.md" in l), "")
    quote(f"cargo test, ledger missing (exit {r.returncode})", msg)
    check(r.returncode != 0 and "rails/research.md must sit beside games/" in msg, "ledger missing: cargo test fails; it does not skip")
    check(src.read_text() == text_, "rails/research.md is unchanged")


colour_rule()
with sync_playwright() as pw:
    browser = pw.chromium.launch()
    guard3(browser)
    storage_blocked_or_full(browser)
    wasm_fails(browser)
    second_tab(browser)
    bad_seed_and_real(browser)
    reduced_motion(browser)
    frame_time(browser)
    browser.close()
overpass()
osm_runs()
ledger()

bad = [w for ok, w in results if not ok]
print(f"{len(results) - len(bad)}/{len(results)} passed")
sys.exit(1 if bad else 0)
