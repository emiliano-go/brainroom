# Implementation references

Two existing Rust/Qt applications were reviewed before starting Brainroom. They are references, not dependencies; nothing is vendored.

## Corcho — primary reference

<https://github.com/emiliano-go/corcho> · MIT · Rust + Qt 6 + `cxx-qt` 0.10

Corcho is the closest architectural match: a native desktop app with a QML UI, a single Rust process, AI provider calls, streaming, a web engine, sync, updates, and Linux packaging.

What to reuse as a pattern:

- **Build wiring:** `build.rs` uses `CxxQtBuilder::new_qml_module(QmlModule::new("Corcho").qml_files(...))`, declares Qt modules (`Quick`, `QuickControls2`, `Network`, `Svg`, `WebEngineQuick`), and compiles the Rust ↔ QML bridge from one `bridge.rs`.
- **One QML-facing controller:** `src/bridge.rs` exposes a single `#[qobject]` controller with `#[qproperty]` values (JSON blobs for complex state), `#[qsignal]` events for streaming (`aiChunk`, `aiDone`, `aiError`), and `#[qinvokable]` commands. QML parses the JSON into view models.
- **Async runtime:** a process-wide `tokio::runtime::Runtime` in `OnceLock`, with QML commands spawning tasks and emitting signals back.
- **AI layer:** `src/ai.rs` is a compact provider abstraction — OpenAI-compatible, Anthropic Messages, and Google Generative AI — with hand-written SSE parsing and per-provider base URLs. This is a good starting shape for Brainroom's `crates/ai`.
- **Storage/model:** `serde` + `serde_json` models; whole-record JSON persistence; clear `model.rs` / `storage.rs` split.
- **Platform features:** single-instance forwarding, deep links, update channels, crash/log handling, low-memory Chromium flags, and lazy web profiles.
- **Packaging:** `packaging/` contains AUR PKGBUILD, AppImage and deb scripts, Flatpak manifest, desktop file, and icon. Useful as a checklist for Brainroom.
- **QML component vocabulary:** `AppButton`, `AppTextField`, `SettingRow`, `ToggleSwitch`, `SelectField`, `SliderField`, `MenuList`, `ContextMenu`, `ToastHost`, `Sidebar`, `SettingsDialog`, `Onboarding`. These are good naming/structure references, not code to copy.
- **MCP server:** Corcho exposes its whole app over MCP (stdio and local HTTP). Brainroom can add the same surface later for automation/testing.

Qt 6.12 and `cxx-qt` 0.10 are installed/available here, matching Corcho's stack.

## Orchard — secondary reference

<https://github.com/SFG5453/Orchard> · AGPL-3.0-or-later · Qt C++ frontend + Rust core over FFI

Orchard is a large native Qt application with a Rust audio core, built with Meson/C++ rather than `cxx-qt`.

What is useful:

- **Process/library boundary:** the Rust core builds as a static library (`crates/orchard-core-ffi`) and the C++/Qt app links it. This is the alternative if `cxx-qt` proves too limiting for a subsystem.
- **Cross-platform build orchestration:** Meson + Cargo, target-specific linking, Android/desktop variants.
- **Service/provider layout:** `providers/`, `services/`, `core/js/` show a clean split between app UI, provider integrations, and hosted services.
- **AGPL project structure:** a reference for notices/third-party licensing and for distributing a large AGPL Qt application.
- **Tests and packaging:** a broad test tree and platform packaging conventions.

Brainroom will stay on the `cxx-qt` model first (Corcho-style). Orchard's C++/FFI bridge remains the fallback if a subsystem needs C++-owned Qt types that `cxx-qt` cannot express cleanly.

## What Brainroom takes from each

- From Corcho: project/build layout, QML controller pattern, provider/SSE shape, packaging checklist, lazy WebEngine approach.
- From Orchard: the C++/Qt ↔ Rust fallback boundary, cross-platform packaging discipline, service/provider separation, AGPL notice hygiene.
- From LearnHarness: behavior, prompts, data contracts, teaching methodology, and the extraction/practice/graph invariants.
