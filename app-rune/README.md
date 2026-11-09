# Brainroom, Rune UI

Native Rune shell for Brainroom. The shared domain crates (`core`, `store`, `ai`, `agent`, `extract`) are unchanged.

## Run

```sh
cargo run -p brainroom-rune
```

## Headless checks (Rune tooling)

```sh
B=target/debug/brainroom-rune

$B --tree /tmp/brainroom-tree.json                     # accessibility tree + bounds
$B --snap /tmp/brainroom-home.png --size 1920x1080 --scale 1
$B --snap /tmp/brainroom-session.png --scene session --size 1920x1080 --scale 1
$B --snap /tmp/brainroom-settings.png --scene settings --size 1920x1080 --scale 1
$B --script app-rune/scripts/quiz.json --scene quiz \
   --snap /tmp/brainroom-quiz-answered.png --at 1.0 --size 1920x1080 --scale 1
$B --script app-rune/scripts/settings.json \
   --snap /tmp/brainroom-settings-providers.png --at 1.0 --size 1920x1080 --scale 1
```

Scenes: `session`, `settings`, `quiz`, `problem`, `streaming`, `math`.

## Math

Math is parsed and rendered locally: `data::parse_markdown` splits display/inline TeX;
`md.rs` builds inline image atoms through `math.rs`, which rasterizes with `katex-native`
and caches by formula/display/size/color. Display math is a centered image block.
Bold/italic spans are tokenized recursively so math inside emphasis renders too.

Upstream math hook request: https://github.com/anomalyco/rune/issues/332

## Parity notes

- Sidebar is 288 pt, matching LearnHarness's 292 pt within 4 pt.
- Session transcript is left-aligned at 32 pt and capped at 1544 pt; the composer and
  bottom control rail are 700 pt, as in LearnHarness.
- Markdown supports headings, paragraphs, bullets, numbered lists, tables, code blocks,
  block quotes, rules, inline code, bold/italic and inline/display LaTeX.
- Sidebar session rows use sans titles and "Waiting for input · date" metadata.
- The fake minimap and waveform meter from the Qt build are gone.
- Settings is a 918x625 modal over the dimmed shell, with LearnHarness's nav order
  (Providers, Agent, Appearance, Graph, Keyboard, General) and the Providers panel reads
  masked key tails from `auth.json`.
- The top-right graph gear opens LearnHarness's render drawer (node/edge/label sizes,
  regions, blur, dilation, edge alpha, bitmap resolution, cluster labels); the sliders
  drive the graph renderer.
- Session bottom matches LearnHarness: reading-thinking panel, collapsed
  "Check your understanding" card, expanded options/confidence/thinking, answered compact
  row, and the single-row token/dictate/model/effort rail. The reply composer hides while
  a quiz is live.
- Usage and spend are read from the last assistant `usage` block.

## Known gaps

- Context denominator is a fixed `202.8k` until the model catalog reports context windows.
- Graph renderer settings are per-session state, not persisted to `settings.json`.
