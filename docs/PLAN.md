# Brainroom — full-parity Rust + Qt plan

Status: Phase 0. Upstream reference: LearnHarness 0.3.9 (AGPL-3.0).

## Goals

- Native **Rust + Qt 6 (Qt Quick/QML)** implementation of LearnHarness.
- No Electron and no Node in the main process.
- Full feature parity: teacher/prober, plan tree, question dock, knowledge graph, extraction, practice, subagents, providers/subscriptions, dictation, PDF, artifacts/demos, settings, updater, packaging.
- Linux first, then macOS and Windows.
- Data-format compatible with LearnHarness (same JSON/JSONL paths) so both apps can share a Syncthing folder during migration.
- Idle RSS target: **120–250 MB** (current Electron baseline: ~543 MB). A lazily spawned browser is allowed for HTML/JS demos and visual verification only.

## Architecture

```
app/            Qt Quick/QML shell, cxx-qt bridge, window/updater/lifecycle
crates/core     contracts, settings, grading, plan tree, coverage, chains, compaction
crates/store    chats, graph JSONL, jobs, practice, settings, auth
crates/ai       provider trait, OpenRouter first, SSE streaming, tool calls, fallback
crates/agent    prober/teacher loop, tools, subagents, extraction, practice, jobs
crates/platform audio, PDF, single-instance, updater, file dialogs, packaging
```

- UI: **Qt Quick/QML + cxx-qt 0.10** (spike against `qmetaobject` 0.2.10).
- Math: **`katex-native`** display lists rendered through QPainter/QML.
- Diagrams: **`flowmaid`** (pure-Rust Mermaid-like engine) with a webview fallback only for unsupported Mermaid syntax.
- Markdown/code: `pulldown-cmark` + `syntect`.
- PDF: `pdfium-render` + `pdfium-bundled`.
- Audio/dictation: `cpal` + `rubato` + Deepgram WebSocket.
- Browser: lazy child process (QtWebEngine or system Chromium) for HTML/JS demos and visual verification; not in the idle path.
- Providers: OpenRouter first, then a `Provider` trait covering the remaining APIs; subscriptions (ChatGPT/Copilot/Grok) after the beta.
- No local HTTP server: QML ↔ Rust via models/signals; optional debug HTTP behind a feature flag.

## Phases and gates

Estimates: solo + AI assistance. Two developers compress ~35–45%.

| Phase | Deliverable | Estimate | Gate |
|---|---|---|---|
| 0 | Workspace + QML shell; OpenRouter stream; Markdown + `katex-native`; one persisted chat; RSS benchmark | 2–4 weeks | Idle RSS < 200 MB; streaming works |
| 1 | Contracts/storage/settings/auth/model catalog/pricing/grading/plan tree; format-compatible | 3–5 weeks | LearnHarness fixtures read/write byte-compatible |
| 2 | Prober/teacher loop; quiz/ask/explain/plan/send_to_user; steering/abort; chat UI | 5–7 weeks | One full lesson end-to-end |
| 3 | Graph renderer/layout/clusters/interactions/chain table | 4–6 weeks | Graph behavior matches LearnHarness |
| 4 | Jobs/subagents: researcher, demo, visual, PDF; browser child; wake/steer | 5–7 weeks | Async job flows and demo verification |
| 5 | Extraction: coverage trigger, extractor/auditor/merger, digest, notes import | 5–7 weeks | Graph written solely by the pipeline |
| 6 | Practice: partitioner/generator/grader + store/UI | 4–5 weeks | Problem sets end-to-end |
| 7 | Providers + subscriptions: expand beyond OpenRouter; OAuth/device code; keys | 6–10 weeks | Existing providers work |
| 8 | Platform: dictation, PDF, updater, packaging, single-instance, macOS/Windows | 4–6 weeks | Installable AppImage/deb + cross-platform smoke |
| 9 | Parity hardening: tests, performance, UI polish, migration/dual-run | 6–8 weeks | Full parity checklist green |

Total: ~12–18 months solo; ~7–10 months with two devs. OpenRouter-only beta: ~4–5 months.

## Decisions

- **v1 beta gate:** OpenRouter-only; add providers incrementally.
- **HTML/JS demos:** lazy browser on demand (QtWebEngine or system Chromium child); idle app is browser-free.
- **Linux first**, macOS/Windows after parity.
- **Data compatibility:** same chat/graph/settings formats and directory layout; dual-run against a copied Syncthing folder.
- **Math/diagrams native:** `katex-native`, `flowmaid`; fallback only where fidelity is insufficient.
- **License:** AGPL-3.0-only; upstream LearnHarness attribution preserved.

## Plan-tool workflow

- Plan task id: `brainroom-full-parity`.
- Create/read with `rbg` `plan_create` / `plan_get` / `plan_tree`.
- Run `plan_audit` with the changed-file list after each milestone (drift check).
- Mark sections complete with `plan_complete` only after the gate passes.
