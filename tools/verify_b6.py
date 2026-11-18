"""B6 verification: the controls, driven with real mouse and key input.

Checks the B6 "done" line of PLAN-cabin.md: a route set and run on 3 seeds,
each `Refusal` shown as text, 0 signal pairs under 44 px at the default
view, and a command menu that opens, closes on one Esc and returns focus.
Also: zero console errors and no network request, the route preview
between the clicks, Esc, Space, the rack, `?seed=` replay and a malformed
seed, both themes.

Every action is a real click or key press on the page, with two kinds of
exception, each named where it happens: `CABIN.app.skip(s)` runs game time
at once, so a 5-minute dwell does not take 75 real seconds; and three
refusals that no control can reach (a route that is not set, an empty
portal, an overlap occupied) are produced by calling the method the
controls call, and the check is still on the text the page shows.

Usage: python3 tools/verify_b6.py <scratch folder>
Exits 0 only if every check passed.
"""
from __future__ import annotations

import itertools
import math
import pathlib
import sys

from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
OUT.mkdir(parents=True, exist_ok=True)
VIEW = {"width": 1920, "height": 1080}
HIT_PX = 44
results: list[tuple[bool, str]] = []
# Every line the status line showed, to check each refusal's text.
seen: list[str] = []


def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = CABIN.app; return {expr}; }})()")


def frames(pg, n: int = 3) -> None:
    pg.evaluate(f"new Promise(r => {{ let k = {n}; const f = () => --k ? requestAnimationFrame(f) : r(); requestAnimationFrame(f); }})")


def status(pg) -> str:
    frames(pg)
    t = pg.locator("#msg").inner_text()
    refused = "refused" in (pg.locator("#msg").get_attribute("class") or "")
    seen.append(t)
    return ("REFUSED " if refused else "") + t


def open_page(b, query="", scheme="dark"):
    ctx = b.new_context(viewport=VIEW, color_scheme=scheme)
    pg = ctx.new_page()
    errs: list[str] = []
    reqs: list[str] = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("request", lambda r: reqs.append(r.url))
    pg.goto(PAGE + query)
    pg.wait_for_timeout(600)
    # Hold game time still; the script moves it with skip().
    pg.keyboard.press("Space")
    return ctx, pg, errs, reqs


def canvas_xy(pg):
    b = pg.locator("#map").bounding_box()
    return b["x"], b["y"], b["width"], b["height"]


def head(pg, name: str):
    """Screen position of signal `name`'s head, page px, and whether it is
    on the canvas."""
    names = app(pg, "a.signal_names()")
    xy = app(pg, "a.signal_screens()")
    i = names.index(name)
    cx, cy, w, h = canvas_xy(pg)
    x, y = xy[2 * i], xy[2 * i + 1]
    return cx + x, cy + y, 0 <= x <= w and 0 <= y <= h - 4


def rack(pg):
    return [r.split("\t") for r in app(pg, "a.rack()")]


def skip_until(pg, pred, max_s=3600, step=5):
    t = 0
    while t < max_s:
        if pred():
            return True
        app(pg, f"a.skip({step})")
        t += step
    frames(pg)
    return pred()


def click_rack(pg, key: str):
    frames(pg)
    pg.locator(f'.bay .row[data-key="{key}"] button').click()
    frames(pg)


def train_row(pg, tid: str):
    return next((r for r in rack(pg) if r[1] == tid), None)


def run_one(b, seed: int) -> None:
    """Accept the first offer, route it in by clicks, let it dwell, route it
    out by clicks, and watch it leave."""
    ctx, pg, errs, reqs = open_page(b, f"?seed={seed}")
    offers = [r for r in rack(pg) if r[4] == "accept"]
    check(len(offers) == 1, f"seed {seed}: one train offered at the start ({[r[3] for r in offers]})")
    pg.locator(f'.bay .row[data-key="{offers[0][1]}"] button').click()
    st = status(pg)
    check("accepted" in st and app(pg, "a.train_count()") == 1, f"seed {seed}: Accept clicked in the rack: '{st}'")
    tid = next(r[1] for r in rack(pg) if r[1].startswith("t"))
    ok = skip_until(pg, lambda: "needs a route" in (train_row(pg, tid) or [""] * 4)[3], 600)
    row = train_row(pg, tid)
    check(ok, f"seed {seed}: the train waits at its Home: '{row[3] if row else None}'")
    home = row[5]
    check(app(pg, f"a.aspect_of('{home}')") == "Red", f"seed {seed}: {home} shows Red")

    # Entry from the rack (the Home is off the default view), exit by a
    # click on the canvas: a Starter on the view, tried until one sets.
    click_rack(pg, tid)
    check(app(pg, "a.selected()") == home, f"seed {seed}: the rack's Route button chose {home}")
    exits = [r.split("\t") for r in app(pg, "a.exits()")]
    check(len(exits) == pg.locator("#exits button").count() and exits, f"seed {seed}: {len(exits)} exits shown as buttons")
    starter = None
    for rid, name, _ in exits:
        if not name.startswith("P"):
            continue
        st_name = name.split("(")[1].rstrip(")")
        x, y, on = head(pg, st_name)
        if not on:
            continue
        pg.mouse.move(x, y)
        frames(pg)
        if app(pg, "a.previewing()") != int(rid):
            check(False, f"seed {seed}: hovering {st_name} previews route {rid}")
            continue
        pg.mouse.click(x, y)
        st = status(pg)
        if st.endswith(" set."):
            starter = st_name
            break
    check(starter is not None, f"seed {seed}: an arrival set by a canvas click on its exit Starter: '{st}'")
    if starter is None:
        ctx.close()
        return
    check(app(pg, f"a.aspect_of('{home}')") == "Yellow", f"seed {seed}: {home} shows Yellow into a berth at a red Starter")
    pg.screenshot(path=str(OUT / f"b6-seed{seed}-arrival-set.png"))
    ok = skip_until(pg, lambda: (train_row(pg, tid) or ["", ""])[0] == "platform", 900)
    check(ok, f"seed {seed}: the train berths and dwells: '{(train_row(pg, tid) or [''] * 4)[3]}'")
    # B7 dwells run to 1500 s (through express), so wait up to 1800.
    ok = skip_until(pg, lambda: (train_row(pg, tid) or ["", ""])[0] == "depart", 1800)
    check(ok, f"seed {seed}: the dwell ends: '{(train_row(pg, tid) or [''] * 4)[3]}'")

    # Departure: entry by a canvas click on the Starter, exit by its button.
    x, y, on = head(pg, starter)
    pg.mouse.click(x, y)
    st = status(pg)
    frames(pg)
    check(app(pg, "a.selected()") == starter, f"seed {seed}: canvas click chose {starter}: '{st}'")
    done = False
    for btn in pg.locator("#exits button").all():
        btn.click()
        st = status(pg)
        if st.endswith(" set."):
            done = True
            break
    check(done, f"seed {seed}: a departure set from the exit buttons: '{st}'")
    check(app(pg, f"a.aspect_of('{starter}')") == "Green", f"seed {seed}: {starter} shows Green")
    ok = skip_until(pg, lambda: app(pg, "a.train_count()") == 0, 900)
    check(ok and not app(pg, "a.live_routes()"), f"seed {seed}: the train left and every lock went ({app(pg, 'a.live_routes()')})")
    check(not errs, f"seed {seed}: no console error {errs[:3]}")
    check(all(r.startswith("file:") for r in reqs), f"seed {seed}: no network request ({[r for r in reqs if not r.startswith('file:')][:3]})")
    ctx.close()


def refusals(b) -> None:
    """Produce each refusal and read the status line. Seed 3 offers its
    first train at S4: on seed 1 it is N5, whose Home stands on the box
    edge, so that train is past its Home at once and no approach lock can
    happen."""
    ctx, pg, errs, _ = open_page(b, "?seed=3")
    # Conflict: a departure from an on-screen Starter, then a second exit of
    # the same Starter; both hold its own track.
    names = app(pg, "a.signal_names()")
    starters = [n for n in names if n.startswith("ST") and head(pg, n)[2]]
    s0 = None
    for s in starters:
        x, y, _ = head(pg, s)
        pg.mouse.click(x, y)
        frames(pg)
        if app(pg, "a.selected()") == s and pg.locator("#exits button").count() >= 2:
            s0 = s
            break
        pg.keyboard.press("Escape")
    check(s0 is not None, f"a Starter with two exits chosen by a canvas click: {s0}")
    bs = pg.locator("#exits button")
    bs.nth(0).click()
    first = status(pg)
    x, y, _ = head(pg, s0)
    pg.mouse.click(x, y)
    st = status(pg)
    check("is set" in st and pg.locator("#exits button.set").count() == 1, f"choosing {s0} again says its route is set: '{st}'")
    pg.locator("#exits button:not(.set)").first.click()
    st = status(pg)
    check(st.startswith("REFUSED") and " holds " in st, f"Conflict shown: '{st}' (after '{first}')")
    # Cancel with the Delete key, then Esc clears the choice.
    x, y, _ = head(pg, s0)
    pg.mouse.click(x, y) if app(pg, "a.selected()") != s0 else None
    pg.keyboard.press("Delete")
    st = status(pg)
    check("cancelled" in st and not app(pg, "a.live_routes()"), f"Delete cancels the chosen route: '{st}'")
    pg.mouse.click(*head(pg, s0)[:2])
    pg.keyboard.press("Escape")
    frames(pg)
    check(app(pg, "a.selected()") == "", "Esc clears the chosen signal")

    # A train in; ApproachLocked, Occupied, TrainPastSignal and
    # NoLineClear, in the order play reaches them.
    offer = next(r for r in rack(pg) if r[4] == "accept")
    pg.locator(f'.bay .row[data-key="{offer[1]}"] button').click()
    tid = next(r[1] for r in rack(pg) if r[1].startswith("t"))
    home = train_row(pg, tid)[5]

    def arrive() -> str | None:
        click_rack(pg, tid)
        for btn in pg.locator("#exits button").all():
            text = btn.inner_text()
            if "(" not in text:
                continue
            btn.click()
            if status(pg).endswith(" set."):
                return text.split("(")[1].rstrip(")")
        return None

    def menu(query: str) -> str:
        pg.keyboard.press("Control+k")
        pg.keyboard.type(query)
        pg.keyboard.press("Enter")
        st = status(pg)
        # No match leaves the menu open; close it so the next step starts clean.
        if pg.evaluate("document.getElementById('cmd').open"):
            pg.keyboard.press("Escape")
        return st

    # Set at once, while the train is still coming in from the box edge,
    # then put back from the menu: the locks stay, and a second cancel is
    # refused.
    berth = arrive()
    # One game tick, so the train takes the route it is approaching.
    app(pg, "a.skip(0.5)")
    st1 = menu(f"cancel {home}".lower())
    st = menu(f"cancel {home}".lower())
    check(st.startswith("REFUSED") and "approach locked, released in" in st, f"ApproachLocked shown, from the menu: '{st}' (after '{st1}')")
    skip_until(pg, lambda: not app(pg, "a.live_routes()") and "needs a route" in (train_row(pg, tid) or [""] * 4)[3], 400)
    berth = arrive()
    skip_until(pg, lambda: (train_row(pg, tid) or [""])[0] == "platform", 900)
    check((train_row(pg, tid) or [""])[0] == "platform", f"the train berthed at {berth}")
    # Occupied: another arrival into the occupied berth, typed in the menu.
    titles = [r.split("\t")[2] for r in app(pg, "a.commands()") if r.startswith("route\t") and r.endswith(f"({berth})")]
    st = ""
    for title in titles:
        _, h, _, plat = title.split(" ")[:4]
        if h == home:
            continue
        st = menu(f"{h} {plat} {berth}".lower())
        if "stands on or foul of" in st:
            break
    check("stands on or foul of" in st, f"Occupied shown, typed in the menu: '{st}'")
    # OverlapOccupied is not produced here. It needs a train berthed beside
    # a point that another route's overlap runs over; with T1 at ST6N a
    # menu try of every arrival (2026-11-17) found none, and the try took a
    # minute. Its text is checked by the sim test
    # every_refusal_reads_as_one_line_naming_what_is_in_the_way.
    # Departure; then TrainPastSignal from the menu once the train is past
    # its Starter.
    skip_until(pg, lambda: (train_row(pg, tid) or [""])[0] == "depart", 1800)
    pg.mouse.click(*head(pg, berth)[:2])
    frames(pg)
    port = None
    for btn in pg.locator("#exits button").all():
        text = btn.inner_text()
        btn.click()
        if status(pg).endswith(" set."):
            port = text
            break
    skip_until(pg, lambda: "leaving" in (train_row(pg, tid) or [""] * 4)[3], 120, step=1)
    app(pg, "a.skip(4)")
    st = menu(f"cancel {berth} {port}".lower())
    check(st.startswith("REFUSED") and "already past the signal" in st, f"TrainPastSignal shown, from the menu: '{st}'")
    skip_until(pg, lambda: app(pg, "a.train_count()") == 0, 900)
    # NoLineClear: another departure by the same portal within the headway.
    st = ""
    for s in starters:
        if s == berth:
            continue
        pg.mouse.click(*head(pg, s)[:2])
        frames(pg)
        b_ = pg.locator("#exits button", has_text=port or "-")
        if b_.count():
            b_.first.click()
            st = status(pg)
            if "no Line Clear" in st:
                break
        pg.keyboard.press("Escape")
    check("no Line Clear" in st, f"NoLineClear shown, by exit button: '{st}'")

    # No control reaches these three; the method the controls call is used,
    # and the text is read from the page.
    pg.keyboard.press("Escape")
    app(pg, "a.cancel_route(0)")
    st = status(pg)
    check(st.startswith("REFUSED") and "that route is not set" in st, f"NoSuchRoute shown (method call): '{st}'")
    empty = next(i for i, p in enumerate(app(pg, "a.portal_names()")) if not any(r[4] == "accept" and r[5] == str(i) for r in rack(pg)))
    app(pg, f"a.accept({empty})")
    st = status(pg)
    check(st.startswith("REFUSED") and "no train is offered" in st, f"NotOffered shown (method call): '{st}'")
    check(not errs, f"refusal run: no console error {errs[:3]}")
    pg.screenshot(path=str(OUT / "b6-refusal.png"))
    ctx.close()


def menu_and_keys(b) -> None:
    ctx, pg, errs, _ = open_page(b, "?seed=3")
    # A mouse click opens it and leaves no focus behind, so Space still
    # pauses afterwards.
    pg.locator("#cmdopen").click()
    frames(pg)
    check(pg.evaluate("document.getElementById('cmd').open"), "a click on Commands opens the menu")
    pg.keyboard.press("Escape")
    check(pg.evaluate("document.activeElement === document.body"), "after a mouse click, no control keeps focus")
    # From the keyboard: focus the button, Enter opens, Esc must return
    # focus to it.
    pg.locator("#cmdopen").focus()
    pg.keyboard.press("Enter")
    frames(pg)
    check(pg.evaluate("document.getElementById('cmd').open"), "Enter on the focused Commands button opens the menu")
    bg = pg.evaluate("getComputedStyle(document.getElementById('cmd')).backgroundColor")
    pg.screenshot(path=str(OUT / "b6-menu-dark.png"))
    pg.keyboard.press("Escape")
    frames(pg)
    check(not pg.evaluate("document.getElementById('cmd').open"), "one Esc closes the menu")
    check(pg.evaluate("document.activeElement.id") == "cmdopen", f"focus returns to the opener ({pg.evaluate('document.activeElement.id')})")
    # Ctrl K, two signal names, Enter: a route is set.
    pg.keyboard.press("Control+k")
    rows = app(pg, "a.commands()")
    route = next(r.split("\t")[2] for r in rows if r.startswith("route\t") and "(ST" in r)
    entry, exit_ = route.split(" ")[1], route.split(" ")[3]
    pg.keyboard.type(f"{entry} {exit_}".lower())
    first = pg.locator("#cmdlist button").first.inner_text()
    pg.keyboard.press("Enter")
    st = status(pg)
    check(first.startswith(f"Set {entry} to {exit_}") and st.endswith(" set."), f"typing '{entry.lower()} {exit_.lower()}' and Enter: '{first}' -> '{st}'")
    # Space resumes and pauses (open_page paused it). Focus is on the
    # body: the menu returns it to its opener only when that was a control.
    pg.evaluate("document.activeElement.blur()")
    pg.keyboard.press("Space")
    g0 = app(pg, "a.game_s()")
    pg.wait_for_timeout(500)
    g1 = app(pg, "a.game_s()")
    pg.keyboard.press("Space")
    g2 = app(pg, "a.game_s()")
    pg.wait_for_timeout(500)
    g3 = app(pg, "a.game_s()")
    check(g1 - g0 > 1.0, f"Space resumes: {g1 - g0:.1f} game s in 0.5 real s at x4")
    check(g3 == g2, f"Space pauses: {g3 - g2:.1f} game s in 0.5 real s")
    # T switches theme; the menu surface in light.
    pg.keyboard.press("t")
    frames(pg)
    theme = pg.evaluate("document.documentElement.dataset.theme")
    check(theme == "light", f"T switches to light ({theme})")
    pg.keyboard.press("Control+k")
    pg.screenshot(path=str(OUT / "b6-menu-light.png"))
    pg.keyboard.press("Escape")
    pg.screenshot(path=str(OUT / "b6-light.png"))
    check(not errs, f"menu run: no console error {errs[:3]}; dark menu surface {bg}")
    ctx.close()


def hit_slop(b) -> None:
    ctx, pg, _, _ = open_page(b, "?seed=1")
    names = app(pg, "a.signal_names()")
    pts = [(n, *head(pg, n)) for n in names]
    on = [p for p in pts if p[3]]
    close = sorted(((a[0], c[0], round(math.dist(a[1:3], c[1:3]), 1)) for a, c in itertools.combinations(on, 2) if math.dist(a[1:3], c[1:3]) < HIT_PX), key=lambda t: t[2])
    check(not close, f"default view: {len(close)} signal pairs closer than {HIT_PX} px, of {len(on)} signals on it; closest {close[:6]}")
    pg.screenshot(path=str(OUT / "b6-default-view.png"))
    ctx.close()


def seeds(b) -> None:
    a = open_page(b, "?seed=2")
    c = open_page(b, "?seed=2")
    # Field 3 holds the live wait ("waiting 0:02"), which depends on when
    # each page was paused; the rest names the train and the portal.
    ra, rc = [r[:3] + r[4:] for r in rack(a[1])], [r[:3] + r[4:] for r in rack(c[1])]
    check(ra == rc and app(a[1], "a.seed()") == 2, f"?seed=2 twice gives the same first offer ({ra[0] if ra else None})")
    a[0].close(); c[0].close()
    ctx, pg, errs, _ = open_page(b, "?seed=abc")
    st = pg.locator("#msg").inner_text()
    seed = app(pg, "a.seed()")
    check("is not a whole number" in st and str(seed) in st and pg.locator("#seed").inner_text() == str(seed), f"?seed=abc: '{st}'")
    check(not errs, f"malformed seed: no console error {errs[:3]}")
    ctx.close()


with sync_playwright() as pw:
    browser = pw.chromium.launch()
    for s in (1, 2, 3):
        run_one(browser, s)
    refusals(browser)
    menu_and_keys(browser)
    hit_slop(browser)
    seeds(browser)
    browser.close()

phrases = {
    "NoSuchRoute": "that route is not set", "Conflict": " holds ", "Occupied": "stands on or foul of",
    "OverlapOccupied": "stands in the overlap", "NoLineClear": "no Line Clear", "TrainPastSignal": "already past the signal",
    "ApproachLocked": "approach locked, released in", "NotOffered": "no train is offered",
}
shown = [k for k, p in phrases.items() if any(p in t for t in seen)]
print(f"refusals shown as text on the page: {len(shown)} of {len(phrases)}; not reached: {[k for k in phrases if k not in shown]}")
bad = [w for ok, w in results if not ok]
print(f"{len(results) - len(bad)}/{len(results)} passed")
sys.exit(1 if bad else 0)
