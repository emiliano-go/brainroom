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

## Known gaps

- Answered quizzes stay as cards; LearnHarness collapses them into the answered card.
- Token usage in the status line is still a placeholder (`0 / 0`).
- Graph still renders in a bounded box rather than full-bleed behind the composer.
- Settings dialog is smaller than LearnHarness's and uses mono section labels.
