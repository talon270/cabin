#!/bin/sh
# Fetch one station's railway ways, nodes and platforms from OpenStreetMap
# into data/<code>.osm.json, with full geometry.
#
#   ./fetch-osm.sh                       New Delhi, the box below
#   ./fetch-osm.sh huk 28.794,77.094,28.8085,77.111
#                                        another station: its code and its
#                                        box as south,west,north,east
#
# Run only to refresh the yard; the game itself never fetches. The raw answer is
# kept so every later extract is reproducible from it. Then rebuild:
#   cargo run -p cabin-osm
#
# The box is the one measured on 2026-11-14 (plan A1): 185 track ways, 79
# switches, 16 platform ways.
#
# CABIN_DATA and OVERPASS_URL exist for the failure drill in tools/verify_b8.py:
# a scratch folder and a canned answer, so the real file is never at risk.
set -eu
cd "$(dirname "$0")"
CODE="${1:-ndls}"
BOX="${2:-28.632,77.210,28.652,77.228}"
case "$CODE" in *[!a-z0-9]*|'') echo "station code must be lower-case letters and digits: '$CODE'" >&2; exit 2;; esac
case "$BOX" in *[!0-9.,]*) echo "box must be south,west,north,east in degrees: '$BOX'" >&2; exit 2;; esac
NAME=$(echo "$CODE" | tr a-z A-Z)
DATA="${CABIN_DATA:-data}"
URL="${OVERPASS_URL:-https://overpass-api.de/api/interpreter}"
mkdir -p "$DATA"
OUT="$DATA/$CODE.osm.json"
tmp="$OUT.tmp"
curl -sS --fail --max-time 180 -A 'cabin-osm/0.1 (offline game, one fetch per station)' \
  --data-urlencode data@- "$URL" -o "$tmp" <<Q
[out:json][timeout:90];
(
  way($BOX)["railway"~"^(rail|platform|siding|yard)\$"];
  way($BOX)["public_transport"="platform"];
  node($BOX)["railway"~"^(switch|railway_crossing|buffer_stop|signal|train_signal|derail)\$"];
);
(._;>;);
out body qt;
Q
# Overpass answers a timeout with HTTP 200 and an empty element list. Keep the
# old file rather than replace a working yard with nothing.
if ! grep -q '"type": "way"' "$tmp"; then
  echo "Overpass returned no ways for $NAME; kept the previous $OUT." >&2
  rm -f "$tmp"; exit 1
fi
# A query that runs out of time or memory can still return part of the
# elements, with a "remark" naming the error. A partial yard is worse than
# the old one, so refuse it too. cabin-osm's comparison is the second check.
if grep -q '"remark"' "$tmp"; then
  echo "Overpass answered with an error remark (a partial answer); kept the previous $OUT:" >&2
  grep -o '"remark"[^}]*' "$tmp" | head -c 300 >&2; echo >&2
  rm -f "$tmp"; exit 1
fi
mv "$tmp" "$OUT"
# cabin-osm needs the box too: a running line that leaves it is a portal.
echo "$BOX" > "$DATA/$CODE.box"
echo "wrote $OUT ($(wc -c < "$OUT") bytes) and $DATA/$CODE.box"
