#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${BIN:-$ROOT/target/debug/brainroom-rune}"
OUT="${OUT:-/tmp/brainroom-rune-parity}"
mkdir -p "$OUT"
"$BIN" --tree "$OUT/tree.json" --size 1920x1080 --scale 1
python3 - "$OUT/tree.json" <<'PY'
import json, sys
tree = json.load(open(sys.argv[1])).get("accessibility", json.load(open(sys.argv[1])))
def keys(node, out):
    if isinstance(node, dict):
        if node.get("key"):
            out.append(node["key"])
        for child in node.get("children", []):
            keys(child, out)
    elif isinstance(node, list):
        for child in node:
            keys(child, out)
    return out
found = set(keys(tree, []))
for key in ("sidebar", "home-settings", "chains", "import-notes", "topic", "graph-search"):
    assert key in found, f"missing accessible key {key}"
print("tree keys ok")
PY
"$BIN" --snap "$OUT/home.png" --size 1920x1080 --scale 1
"$BIN" --snap "$OUT/session.png" --scene session --size 1920x1080 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/quiz.json" --scene quiz \
    --snap "$OUT/quiz-answered.png" --at 1.0 --size 1920x1080 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/settings.json" \
    --snap "$OUT/settings-providers.png" --at 1.0 --size 1920x1080 --scale 1
echo "parity artifacts: $OUT"
