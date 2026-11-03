# Brainroom — agent instructions

## Attribution

Brainroom is based on and inspired by **LearnHarness** (AGPL-3.0). Keep the attribution in `README.md`, `NOTICE.md`, and `LICENSE` intact. Any code adapted from LearnHarness stays AGPL-3.0. Corcho (MIT) and Orchard (AGPL-3.0-or-later) are implementation references; if code is adapted from either, add the notice to `NOTICE.md` and the affected files.

## Product constraints

- Native **Rust + Qt 6 (Qt Quick/QML)** main process. No Electron, no Node/Next in the idle path.
- A browser child is allowed **only** for HTML/JS demos and visual verification, lazily spawned.
- Preserve LearnHarness data compatibility (chat JSON, graph JSONL, settings, auth) so both apps can share a data folder.
- Linux first; macOS/Windows after parity.
- Keep the AGPL-3.0-only license.

## Plan and drift checks

- Plan store: `task_id = brainroom-full-parity` for root `~/Documents/GitHub/brainroom`.
- Use the `rbg` plan tools: `plan_get` / `plan_tree` to read, `plan_modify` to declare scope (`files`, `acceptance`), `plan_check` for gates, `plan_complete` only after a milestone passes.
- Run `plan_audit` after each milestone (periodic drift check). Declare section scope before implementing; the audit treats undeclared scope as a warning.
- Note: once a milestone is committed, the git-based audit sees no working-tree diff and may report planned files as pending. For post-commit confirmation, use `plan_check` and note the commit hash; the next milestone's working-tree diff drives the audit.

## Gates

```bash
cargo fmt --all -- --check
cargo clippy -p brainroom -- -D warnings
cargo build
cargo test
```

Add QML smoke once the first real UI lands. Keep `target/`, `.qmlls.ini`, data, and secrets out of git.
