"""B10 verification (PLAN-redesign.md B3, B4): the ladder and the guide.

A fresh profile on a bare URL opens level 1, Holambi Kalan. The script then
plays the level by reading the guide line and doing only what it says, with
real clicks: on "Press Accept" or "press Route" it clicks the outlined
button, on "Click <signal> on the panel" it clicks that signal's head, on
"Choose ..." it clicks the first exit button under the panel. When the line
says there is nothing to do, `CABIN.app.skip(10)` runs 10 game seconds at
once, the one exception, so a 90 s dwell does not take 22 real seconds.
Then: progress is saved under schema 3; the level's button opens level 2; a
bare URL now opens level 2; level 3 is locked in the command menu until
level 2 is done; a schema 2 store migrates and keeps its bests; an old
replay link (`?seed=1`) still opens NDLS; no console error, no network
request. The guide's Hide button hides it, the choice survives a reload,
and the command menu brings it back. On every station's default view, each
platform has a box, no two boxes overlap, and no head, name or track pixel
stands inside one past its name and timer.

Usage: python3 tools/verify_b10.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import json
import pathlib
import re
import sys

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
OUT.mkdir(parents=True, exist_ok=True)
results: list[tuple[bool, str]] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg) -> None:
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")


def stored(pg):
    raw = pg.evaluate("localStorage.getItem('cabin.v1')")
    return json.loads(raw) if raw else None


def open_page(b, query: str = "", seed_store: str | None = None):
    ctx = b.new_context(viewport={"width": 1920, "height": 1080})
    if seed_store is not None:
        ctx.add_init_script(f"if (!sessionStorage.getItem('seeded')) {{ localStorage.setItem('cabin.v1', {json.dumps(seed_store)}); sessionStorage.setItem('seeded', '1'); }}")
    pg = ctx.new_page()
    errs: list[str] = []
    reqs: list[str] = []
    pg.on("pageerror", lambda e: errs.append(str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("request", lambda r: reqs.append(r.url))
    pg.goto(PAGE + query)
    pg.wait_for_timeout(700)
    return ctx, pg, errs, reqs


def play_by_the_guide(pg, max_steps: int = 400) -> list[str]:
    """Follow the guide line until it says the level is done."""
    box = pg.locator("#map").bounding_box()
    said: list[str] = []
    for _ in range(max_steps):
        frames(pg)
        g = pg.text_content("#guidetext")
        if not said or said[-1] != g:
            said.append(g)
        if g.startswith("Level done") or g.startswith("Press Again"):
            return said
        m = re.search(r"Click (\S+) on the panel", g)
        if "Press Accept" in g:
            pg.locator(".next").click()
        elif m:
            names = app(pg, "a.signal_names()")
            xy = app(pg, "a.signal_screens()")
            i = names.index(m.group(1))
            pg.mouse.click(box["x"] + xy[2 * i], box["y"] + xy[2 * i + 1])
        elif g.startswith("Choose"):
            pg.locator("#exits button").first.click()
        else:
            app(pg, "a.skip(10)")
    return said


with sync_playwright() as pw:
    b = pw.chromium.launch()

    # ---- Level 1 on a fresh profile, played by the guide ------------------
    ctx, pg, errs, reqs = open_page(b)
    check(pg.text_content("#brand") == "Cabin · HUK", f"a bare URL on a fresh profile opens level 1: {pg.text_content('#brand')!r}")
    check(pg.locator("#guide").is_visible(), "the guide line shows on a level")
    check("all invented" in pg.text_content("#msg"), f"the level says its trains are invented: {pg.text_content('#msg')!r}")
    check(pg.locator(".bay .none").count() == 3, f"each empty bay says so ({pg.locator('.bay .none').count()} of 3)")
    pg.screenshot(path=str(OUT / "b10-huk-start.png"))
    said = play_by_the_guide(pg)
    for line in said:
        print("  guide:", line, flush=True)
    check(said[-1].startswith("Level done"), f"level 1 finished by following the guide alone, {len(said)} distinct lines")
    check(any("Press Accept" in s for s in said) and any(re.search(r"Click \S+ on the panel", s) for s in said) and any(s.startswith("Choose") for s in said),
          "the guide named Accept, a signal to click, and an exit to choose")
    st = stored(pg)
    check(st and st.get("v") == 3 and (st.get("progress") or {}).get("huk", {}).get("done") is True, f"progress saved under schema 3: {st and st.get('progress')}")
    check(app(pg, "a.score()")[0] == 3 and "Level 1 done" in pg.text_content("#msg"), f"3 of 3 handled: {pg.text_content('#msg')!r}")
    pg.screenshot(path=str(OUT / "b10-huk-done.png"))
    pg.locator("#exits button").first.click()
    pg.wait_for_timeout(700)
    check(pg.text_content("#brand") == "Cabin · NUR", f"the level's button opens level 2: {pg.text_content('#brand')!r}")
    pg.goto(PAGE)
    pg.wait_for_timeout(700)
    check(pg.text_content("#brand") == "Cabin · NUR", "a bare URL now opens level 2, the first not done")
    pg.keyboard.press("Control+k")
    frames(pg)
    items = pg.locator("#cmdlist").inner_text()
    check("Level 1: Holambi Kalan (done)" in items and "Level 3: Delhi Sarai Rohilla (locked" in items, "the menu marks level 1 done and level 3 locked")
    pg.keyboard.press("Escape")
    check(not errs, f"no console error in the ladder run {errs[:2]}")
    check(all(u.startswith("file://") for u in reqs), f"no network request ({len(reqs)} requests, all file://)")
    ctx.close()

    # ---- The guide can be hidden, and the choice is kept -------------------
    ctx, pg, errs, _ = open_page(b)
    pg.click("#guidehide")
    frames(pg)
    hid = not pg.locator("#guide").is_visible() and pg.locator(".next").count() == 0
    check(hid and stored(pg)["settings"].get("guide") is False, f"Hide hides the guide and its outline, and saves it: {stored(pg)['settings']}")
    pg.reload()
    pg.wait_for_timeout(700)
    check(not pg.locator("#guide").is_visible(), "the guide stays hidden after a reload")
    pg.keyboard.press("Control+k")
    frames(pg)
    pg.locator("#cmdlist button", has_text="Show the guide line").click()
    frames(pg)
    check(pg.locator("#guide").is_visible() and stored(pg)["settings"].get("guide") is True, "the command menu shows it again")
    check(not errs, f"no console error hiding the guide {errs[:2]}")
    ctx.close()

    # ---- Platform boxes on every station --------------------------------
    BOXES = """() => {
      const a = CABIN.app, bx = a.platform_boxes(), n = bx.length / 4;
      const cv = document.getElementById('map'), k = cv.width / cv.getBoundingClientRect().width;
      const d = cv.getContext('2d').getImageData(0, 0, cv.width, cv.height);
      let missing = 0, overlap = 0, inside = 0;
      for (let i = 0; i < n; i++) {
        const [l, r, t, b] = bx.slice(4 * i, 4 * i + 4);
        if (isNaN(l) || r - l < 40) { missing++; continue; }
        for (let j = i + 1; j < n; j++) {
          const [l2, r2, t2, b2] = bx.slice(4 * j, 4 * j + 4);
          if (l < r2 && l2 < r && t < b2 && t2 < b) overlap++;
        }
        // Past the name and timer (70 px), the box's inside is background.
        for (let y = Math.ceil(t + 2); y <= b - 2; y++) for (let x = Math.ceil(l + 70); x <= r - 2; x++) {
          const p = 4 * (Math.round(y * k) * d.width + Math.round(x * k));
          if (d.data[p] + d.data[p + 1] + d.data[p + 2] >= 40) inside++;
        }
      }
      return {n, missing, overlap, inside};
    }"""
    for code in ["huk", "nur", "dee", "ndls"]:
        ctx, pg, errs, _ = open_page(b, f"?station={code}")
        r = pg.evaluate(BOXES)
        check(r["n"] == app(pg, "a.platform_count()") and r["missing"] == 0 and r["overlap"] == 0 and r["inside"] == 0,
              f"{code}: every platform has a box, none overlap, nothing inside one: {r}")
        pg.screenshot(path=str(OUT / f"b10-boxes-{code}.png"))
        ctx.close()

    # ---- Schema 2 store migrates ------------------------------------------
    v2 = {"v": 2, "settings": {}, "bests": {"light": {"handled": 4, "delay_per_train": 12.5, "put_backs": 1}}}
    ctx, pg, errs, _ = open_page(b, "", json.dumps(v2))
    st = stored(pg)
    check(st and st.get("v") == 3 and st.get("progress") == {} and st.get("bests") == v2["bests"], f"a schema 2 store migrates to 3 and keeps its bests: {st}")
    check(pg.text_content("#brand") == "Cabin · HUK" and not errs, "a migrated profile with no progress opens level 1")
    ctx.close()

    # ---- Old replay links still open NDLS ---------------------------------
    ctx, pg, errs, _ = open_page(b, "?seed=1")
    check(pg.text_content("#brand") == "Cabin · NDLS" and app(pg, "a.platform_count()") == 16, "an old `?seed=1` link opens NDLS, 16 platforms")
    check(not pg.locator("#guide").is_visible(), "no guide line at NDLS")
    ctx.close()
    ctx, pg, errs, _ = open_page(b, "?station=xyz")
    check(pg.text_content("#brand") == "Cabin · HUK" and 'Station "xyz" is not one of' in pg.text_content("#msg"), f"an unknown station is said and level 1 plays: {pg.text_content('#msg')!r}")
    ctx.close()
    b.close()

failed = [w for ok, w in results if not ok]
print(f"RESULT {len(results) - len(failed)}/{len(results)} passed", flush=True)
sys.exit(1 if failed else 0)
