#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${BIN:-$ROOT/target/debug/brainroom-rune}"
OUT="${OUT:-/tmp/brainroom-rune-parity}"
mkdir -p "$OUT"

assert_tree() {
  local tree="$1"; shift
  python3 - "$tree" "$@" <<'PY'
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
for key in sys.argv[2:]:
    assert key in found, f"missing accessible key {key}"
print("tree keys ok:", " ".join(sys.argv[2:]))
PY
}

"$BIN" --tree "$OUT/tree-home.json" --size 1920x1080 --scale 1
assert_tree "$OUT/tree-home.json" sidebar home-settings chains import-notes topic graph-search
"$BIN" --script "$ROOT/app-rune/scripts/graph-settings.json" \
    --tree "$OUT/tree-graph-settings.json" --at 1.0 --size 1920x1080 --scale 1
assert_tree "$OUT/tree-graph-settings.json" graph-settings gs-node-size gs-edge-alpha
"$BIN" --scene session --script "$ROOT/app-rune/scripts/quiz-open.json" \
    --tree "$OUT/tree-session.json" --at 1.0 --size 1920x1080 --scale 1
assert_tree "$OUT/tree-session.json" transcript reading-thinking

"$BIN" --snap "$OUT/home.png" --size 1920x1080 --scale 1
"$BIN" --snap "$OUT/session.png" --scene session --size 1920x1080 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/quiz-open.json" --scene session \
    --snap "$OUT/session-quiz-open.png" --at 1.0 --size 1366x768 --scale 1
"$BIN" --snap "$OUT/settings.png" --scene settings --size 1366x768 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/graph-settings.json" \
    --snap "$OUT/graph-settings.png" --at 1.0 --size 1366x768 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/quiz.json" --scene quiz \
    --snap "$OUT/quiz-answered.png" --at 1.0 --size 1920x1080 --scale 1
"$BIN" --script "$ROOT/app-rune/scripts/settings.json" \
    --snap "$OUT/settings-providers.png" --at 1.0 --size 1920x1080 --scale 1
echo "parity artifacts: $OUT"
