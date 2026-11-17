# tools

Playwright scripts that drive `dist/index.html` with real clicks, drags and
wheel turns. Run `./build.sh` first: a script run on an old page proves
nothing about the change (`GUIDE-games.md` F3). Give each a scratch folder for
its screenshots; each exits 0 only if every check passed.

| Script | Covers | Last run |
|---|---|---|
| `verify_b2.py` | Skeleton: fresh, seeded, corrupt and future-version profiles; both themes at 1920 px; no network request; yard drawn; pan 1:1, rubber band and spring back, zoom about the cursor, reduced-motion zoom; skeleton train and clock run | 2026-11-16: 37/37 passed in 15 s, again after B3 (yard format 2) |

```sh
./build.sh
python3 tools/verify_b2.py /tmp/cabin-shots
```
