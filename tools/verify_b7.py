"""B7 verification: modes, the score, the end of a shift and bests.

Checks the B7 "done" line of PLAN-cabin.md on the built page: a best
survives a reload. Also: `?mode=` picks the shift and a malformed one is
said; the header shows the mode, the shift's window, the clock from 06:30
and the three numbers; delay rises while a train stands at a red Home; a
put-back by the Cancel route button counts; a handled train counts; the
shift ends at 08:00, pauses and saves; a best another tab saved is kept
(per field); a store that cannot be read, when cabin.v1.corrupt already
holds another copy, is left untouched and the result says bests are not
saved; Practice shows no score and never ends; rack
rows carry real train numbers and names; both themes; zero console
errors; no network request.

Every control is a real click, with one named exception copied from
`verify_b6.py`: `CABIN.app.skip(s)` runs game time at once, so a 90-minute
shift does not take 22.5 real minutes. A second tab is stood in for by
writing `cabin.v1` from the test, which is what another tab's save does.

Usage: python3 tools/verify_b7.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import json
import pathlib
import re
import sys

from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
OUT.mkdir(parents=True, exist_ok=True)
VIEW = {"width": 1920, "height": 1080}
SHIFT_S = 90 * 60
results: list[tuple[bool, str]] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg, n: int = 3) -> None:
    pg.evaluate(f"new Promise(r => {{ let k = {n}; const f = () => --k ? requestAnimationFrame(f) : r(); requestAnimationFrame(f); }})")


def text(pg, sel: str) -> str:
    frames(pg)
    return pg.locator(sel).inner_text()


def store(pg):
    raw = pg.evaluate("localStorage.getItem('cabin.v1')")
    try:
        return json.loads(raw) if raw else None
    except ValueError:
        return raw


def rack(pg):
    return [r.split("\t") for r in app(pg, "a.rack()")]


def train_row(pg, tid: str):
    return next((r for r in rack(pg) if r[1] == tid), None)


def skip_until(pg, pred, max_s=3600, step=5):
    t = 0
    while t < max_s:
        if pred():
            return True
        app(pg, f"a.skip({step})")
        t += step
    frames(pg)
    return pred()


class Tab:
    def __init__(self, ctx, query: str, pause: bool = True):
        self.pg = ctx.new_page()
        self.errs: list[str] = []
        self.reqs: list[str] = []
        self.pg.on("pageerror", lambda e: self.errs.append("PAGEERROR " + str(e)))
        self.pg.on("console", lambda m: m.type == "error" and self.errs.append(m.text))
        self.pg.on("request", lambda r: self.reqs.append(r.url))
        self.pg.goto(PAGE + query)
        self.pg.wait_for_timeout(600)
        if pause:
            # Hold game time still; the script moves it with skip().
            self.pg.keyboard.press("Space")

    def clean(self, what: str) -> None:
        check(not self.errs, f"{what}: no console error {self.errs[:3]}")
        check(all(r.startswith("file:") for r in self.reqs), f"{what}: no network request {[r for r in self.reqs if not r.startswith('file:')][:3]}")


def click_exit_until_set(pg, want_berth: bool) -> str:
    """Click the chosen entry's exit buttons, berths or portals, until one sets."""
    for btn in pg.locator("#exits button").all():
        name = btn.inner_text()
        if name.upper().startswith("CANCEL") or name.upper().startswith("P") != want_berth:
            continue
        btn.click()
        st = text(pg, "#msg")
        if st.endswith(" set."):
            return st
    return text(pg, "#msg")


def a_scored_shift(b) -> None:
    """Light, seed 1: the header, delay, a put-back, a handled train, the
    end of the shift, the saved best, and the best after a reload."""
    ctx = b.new_context(viewport=VIEW, color_scheme="dark")
    t = Tab(ctx, "?mode=light&seed=1")
    pg = t.pg
    check(text(pg, "#mode") == "light" and text(pg, "#window") == "06:30-08:00", f"header: mode '{text(pg, '#mode')}', window '{text(pg, '#window')}'")
    # The page runs 600 ms (2.4 game s) before the script pauses it.
    check(re.fullmatch(r"06:30:0\d", text(pg, "#clock")) is not None, f"the clock starts at 06:30: '{text(pg, '#clock')}'")
    # The first offer waits through those 2.4 s: delay may read 0.1.
    check((text(pg, "#handled"), text(pg, "#putbacks")) == ("0", "0") and float(text(pg, "#delay")) <= 0.1, f"the three numbers start at 0: {text(pg, '#handled')}, {text(pg, '#delay')}, {text(pg, '#putbacks')}")
    offer = next(r for r in rack(pg) if r[4] == "accept")
    check(re.fullmatch(r"\d{5}", offer[2]) is not None and len(offer[6]) > 5, f"the offer carries a real number and name: {offer[2]} '{offer[6]}'")
    title = pg.locator(f'.bay .row[data-key="{offer[1]}"]').get_attribute("title")
    check(title == offer[6], f"the rack row's title is the train's name: '{title}'")
    pg.screenshot(path=str(OUT / "b7-light-start-dark.png"))

    pg.locator(f'.bay .row[data-key="{offer[1]}"] button').click()
    st = text(pg, "#msg")
    check(st.startswith(offer[2] + " accepted"), f"accepted by its number: '{st}'")
    tid = next(r[1] for r in rack(pg) if r[1].startswith("t"))
    check(skip_until(pg, lambda: "needs a route" in (train_row(pg, tid) or [""] * 4)[3], 600), "the train waits at its Home")
    # "needs a route" shows while it still runs in; give it time to stand.
    # Trains offered meanwhile add their own wait, so this is a floor; the
    # exact count is the sim test's (b7_traffic.rs).
    app(pg, "a.skip(240)")
    d0 = float(text(pg, "#delay"))
    app(pg, "a.skip(60)")
    d1 = float(text(pg, "#delay"))
    check(d1 - d0 >= 0.95, f"a minute standing at a red Home adds at least a minute of delay: {d0} -> {d1}")

    # Route in, then put it back with the Cancel route button, then again.
    pg.locator(f'.bay .row[data-key="{tid}"] button').click()
    frames(pg)
    st = click_exit_until_set(pg, True)
    check(st.endswith(" set."), f"an arrival set from the exit buttons: '{st}'")
    pg.locator(f'.bay .row[data-key="{tid}"] button').click()
    frames(pg)
    pg.get_by_role("button", name="Cancel route").click()
    st = text(pg, "#msg")
    check("put back" in st or "cancelled" in st, f"Cancel route clicked: '{st}'")
    check(text(pg, "#putbacks") == "1", f"a route cancelled in the face of a train is a put-back: {text(pg, '#putbacks')}")
    # Approach locking may hold the route; wait for it, then set it again.
    pg.locator(f'.bay .row[data-key="{tid}"] button').click()
    frames(pg)
    ok = skip_until(pg, lambda: click_exit_until_set(pg, True).endswith(" set."), 300, step=10)
    check(ok, f"the arrival set again: '{text(pg, '#msg')}'")
    check(skip_until(pg, lambda: (train_row(pg, tid) or [""])[0] == "depart", 1800), "the train berths and its dwell ends")
    pg.locator(f'.bay .row[data-key="{tid}"] button').click()
    frames(pg)
    st = click_exit_until_set(pg, False)
    check(st.endswith(" set."), f"a departure set from the exit buttons: '{st}'")
    check(skip_until(pg, lambda: app(pg, "a.train_count()") == 0, 900), "the train left")
    check(text(pg, "#handled") == "1", f"one train handled: {text(pg, '#handled')}")

    # Run out the shift with nobody at the panel.
    # Resume first, so the pause below is the page's own.
    pg.keyboard.press("Space")
    left = SHIFT_S - app(pg, "a.game_s()")
    app(pg, f"a.skip({left + 1})")
    st = text(pg, "#msg")
    check(st.startswith("Shift over at 08:00: 1 train handled") and "New best" in st, f"the shift ends at 08:00: '{st}'")
    check(app(pg, "a.paused()") and text(pg, "#pause").lower() == "resume", "the page pauses at the end")
    delay = float(text(pg, "#delay"))
    best = (store(pg) or {}).get("bests", {}).get("light")
    check(best == {"handled": 1, "delay_min": delay, "put_backs": 1}, f"saved under bests.light: {best}")
    check(text(pg, "#b-handled") == "best 1" and text(pg, "#b-putbacks") == "best 1", f"header bests: '{text(pg, '#b-handled')}', '{text(pg, '#b-putbacks')}'")
    # The score stops: playing on changes nothing.
    pg.keyboard.press("Space")
    app(pg, "a.skip(600)")
    check(text(pg, "#delay") == f"{delay:.1f}" and text(pg, "#handled") == "1", "the numbers stop after the shift")
    pg.screenshot(path=str(OUT / "b7-light-over-dark.png"))
    t.clean("scored shift")

    # Done-when: a best survives reload.
    pg.reload()
    pg.wait_for_timeout(600)
    frames(pg)
    check(text(pg, "#b-handled") == "best 1" and text(pg, "#b-delay") == f"best {delay:.1f}" and text(pg, "#b-putbacks") == "best 1",
          f"after a reload: '{text(pg, '#b-handled')}', '{text(pg, '#b-delay')}', '{text(pg, '#b-putbacks')}'")

    # A second tab saved a better handled and worse others since this one
    # loaded. This tab's next result must keep the other tab's handled.
    pg.keyboard.press("Space")
    other = store(pg)
    other["bests"]["light"] = {"handled": 9, "delay_min": 9999.0, "put_backs": 5}
    pg.evaluate(f"localStorage.setItem('cabin.v1', {json.dumps(json.dumps(other))})")
    app(pg, f"a.skip({SHIFT_S + 1})")
    frames(pg)
    merged = store(pg)["bests"]["light"]
    check(merged["handled"] == 9 and merged["delay_min"] == delay and merged["put_backs"] == 0,
          f"per-field merge with another tab's save: {merged}")
    ctx.close()


def light_theme_and_bad_mode(b) -> None:
    ctx = b.new_context(viewport=VIEW, color_scheme="light")
    t = Tab(ctx, "?mode=rush&seed=4")
    st = text(t.pg, "#msg")
    check('Mode "rush" is not one of' in st and text(t.pg, "#mode") == "normal", f"?mode=rush: '{st}', playing {text(t.pg, '#mode')}")
    check(t.pg.evaluate("document.documentElement.dataset.theme") == "light", "light theme from the system")
    t.pg.screenshot(path=str(OUT / "b7-normal-start-light.png"))
    t.clean("bad mode, light")
    ctx.close()


def practice(b) -> None:
    ctx = b.new_context(viewport=VIEW, color_scheme="dark")
    t = Tab(ctx, "?mode=practice&seed=2")
    pg = t.pg
    check(not pg.locator("#scores").is_visible() and text(pg, "#window") == "not scored", "Practice hides the score")
    app(pg, f"a.skip({SHIFT_S + 600})")
    st = text(pg, "#msg")
    check("Shift over" not in st and "practice" not in ((store(pg) or {}).get("bests") or {}), f"Practice never ends or saves: '{st}'")
    t.clean("practice")
    ctx.close()


def unreadable_store(b) -> None:
    ctx = b.new_context(viewport=VIEW, color_scheme="dark")
    # cabin.v1.corrupt already holds another copy, so Guard 3 (B8) cannot
    # move this one aside: nothing may be saved. verify_b8.py covers the
    # case where it can.
    ctx.add_init_script("if (!sessionStorage.getItem('seeded')) { localStorage.setItem('cabin.v1', 'not json'); localStorage.setItem('cabin.v1.corrupt', 'older'); sessionStorage.setItem('seeded', '1'); }")
    t = Tab(ctx, "?mode=peak&seed=1")
    pg = t.pg
    app(pg, f"a.skip({SHIFT_S + 1})")
    st = text(pg, "#msg")
    check("Bests are not saved in this browser" in st, f"unreadable store, end of shift: '{st}'")
    check(store(pg) == "not json", "the unreadable value is left untouched")
    check(t.pg.locator("#banner").is_visible(), "the banner says why")
    ctx.close()


with sync_playwright() as pw:
    browser = pw.chromium.launch()
    a_scored_shift(browser)
    light_theme_and_bad_mode(browser)
    practice(browser)
    unreadable_store(browser)
    browser.close()

bad = [w for ok, w in results if not ok]
print(f"{len(results) - len(bad)}/{len(results)} passed")
sys.exit(1 if bad else 0)
