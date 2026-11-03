# Feature inventory and migration status

Source of truth for parity: **LearnHarness 0.3.9** (`~/Documents/GitHub/learnharness`). This is the full migration checklist. Status reflects Brainroom, not LearnHarness.

Legend: **UI** = Qt Quick surface, **Logic** = Rust behavior/data, **Tests** = automated coverage.

## 1. Shell and lifecycle

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Frameless window / custom chrome | `electron/main.cjs`, `components/sidebar.tsx` | UI done (traffic dots, drag, sidebar) | titlebar edge cases |
| Single instance | `electron/main.cjs` | not started | `QLocalServer` / lock file |
| Deep links | `electron/deeplink.cjs` | not started | URL scheme handler |
| Updates / channels | `electron/main.cjs`, `update-banner.tsx` | not started | `self_update` + banner |
| Logs / crash handling | `src/store/auth.ts`, electron logs | not started | tracing + crash report |
| Packaging | `electron-builder.yml`, `packaging/` | not started | AppImage/deb, icons, desktop |
| MCP / automation | (not upstream; Corcho reference) | not started | optional later |

## 2. Home and knowledge graph

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Full-bleed graph | `components/graph-canvas.ts` | UI static (Canvas), pan/zoom | port derivation + real data |
| Cluster glows / hulls | `graph-canvas.ts` | glows done; hulls not | hulls, cluster colors |
| Force layout | d3-force in `graph-canvas.ts` | static generated layout | deterministic Rust force layout |
| Node click / hover | `graph-canvas.ts`, `graph-view.tsx` | click/hover + selection label done | node detail panel, actions |
| Node rename | `graph-view.tsx` | not started | context menu + store |
| Node delete | `delete-node-dialog.tsx` | not started | confirm dialog + ledger rewrite |
| Chain table | `chain-table.tsx` | not started | QML table + chains |
| Search nodes | home top bar | UI only (field) | filter model |
| Import notes | `notes-import.tsx`, `POST /api/graph/import` | UI button only | file picker + pipeline |
| Cost readout | `stats-bar.tsx` | static `$0.49` | real pricing |
| Graph render settings | `render-settings.ts` | not started | settings + persistence |
| Positions / layout settle | `layout.ts`, `positions.json` | not started | Rust layout + JSON |
| Graph as teacher context | `buildGraphContext` | not started | prompt injection |
| Graph master switch | `graph.enabled` | not started | settings + gates |

## 3. Sessions and lesson UI

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Session list/filter | `sidebar.tsx` | UI static | model + store + search |
| New teach / practice | `start-session.ts`, `minimal-home.tsx` | buttons only | create chat JSON |
| Rename / delete | `session` routes | not started | store + dialogs |
| Phase badges / status | `phases.ts` | static statuses | derive from record |
| Background agents section | `activity-dock.tsx` | UI static | jobs feed |
| Probe → plan → teach phases | `phases.ts`, `chat-handler.ts` | not started | state machine |
| Plan tree authoring | `plan_init`, `plan-block.tsx`, `plan-window.tsx` | not started | parser + QML tree |
| Question dock (MC) | `question-dock.tsx` | UI static | selection, grading |
| Free-recall explain | `explain` tool | not started | tool + UI |
| Reveal/collapse check | `questionNeedsReveal` | not started | UI state |
| Confidence slider | `confidence-rail.tsx` | UI done | persist in answer |
| Thinking + reading-thinking | `reading-thinking.tsx` | UI static | capture in answer |
| Dictation in dock | `dictation.tsx` | not started | cpal + Deepgram |
| Composer / steering | `composer.tsx`, `session-core.ts` | home composer only | session composer |
| Abort / retry | `abort-turn-dialog.tsx` | Stop button static | abort plumbing |
| Compaction | `compaction.ts`, divider | not started | port cut/summary |
| Stats / cost / model pills | `stats-bar.tsx` | UI static | real usage |
| Model / thinking pickers | `model-picker.tsx` | UI pills only | menus + settings |
| Session resume | chat JSON replay | not started | native record replay |
| Session find | `session-find.tsx` | not started | find UI |

## 4. Agents and tools

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Prober agent | `teacher-agent.ts`, `prompts/prober/*` | not started | Rust agent loop |
| Teacher agent | `teacher-agent.ts`, `prompts/teacher/*` | not started | Rust agent loop |
| Prompt fragments / overrides | `prompts/*`, `prompt-overrides.ts` | not started | port loader + UI |
| `quiz` / `probe` | `tools.ts`, `question-core.ts` | not started | schema + grading |
| `ask_user_question` | `tools.ts` | not started | tool + dock |
| `explain` / `recall` | `tools.ts` | not started | tool + grading |
| `probe_done` | `tools/probe.ts` | not started | assessment tool |
| `plan_init` / plan marks | `tools/plan.ts` | not started | mermaid-subset parser |
| `send_to_user` | `tools/send-to-user.ts` | not started | tool + render |
| `read` / `ls` / `find` / `grep` | `tools/*` | not started | Rust fs tools |
| Graph tools (`list_sessions`, `list_propositions`) | `tools/graph.ts` | not started | gate + store |
| Tool argument healing | `argument-heal.ts` | not started | port heal + tests |
| Tool loop / streaming | `pi-agent-core`, `pi-ai` | not started | Rust loop + SSE |

## 5. Jobs and subagents

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Async job protocol | `jobs.ts`, `contracts.ts` | not started | Rust job registry/queue |
| Researcher | `web-search`, `web-fetch`, researcher prompt | not started | HTTP + summary |
| Demo builder | `demo-builder`, browser verify | not started | HTML + lazy browser |
| Visual maker | `visual-maker.ts`, `headless.ts` | not started | SVG + raster + vision |
| PDF reader | `pdf-reader.ts`, `pdfjs` | not started | `pdfium-render` |
| Activity dock | `activity-dock.tsx` | not started | job events |
| Wake / steer delivery | `jobs.ts` | not started | exactly-once steer |
| Crash recovery | job sweeps | not started | boot sweep |

## 6. Extraction and graph pipeline

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Coverage trigger | `coverage.ts`, `coverage.jsonl` | not started | token fingerprints |
| Extractor | `subagents/extraction.ts` | not started | prompts + submit tool |
| Auditor | `subagents/extraction.ts` | not started | graph tools + identity |
| Merger | `mergeFindings`, chains | not started | deterministic merger |
| Chains as truth | `chains.jsonl` | not started | schema + dedupe |
| Digest | `sourceDigest` | not started | per-source digest |
| Notes import | `POST /api/graph/import` | not started | front door |
| Layout settle | `layout.ts` | not started | Rust settle |
| Clusters (birth session) | `clusters.ts` | not started | read-side derivation |

## 7. Practice

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Problem sets | `practice/` store | not started | `serde` store |
| Partitioner | practice agents | not started | prompt + loop |
| Generators | practice pipeline | not started | per-problem jobs |
| Graders | practice pipeline | not started | score-only grading |
| Practice UI | `practice-dock.tsx`, cards | not started | QML pages |
| Activity rows | practice registry | not started | dock events |
| Crash sweep | boot sweep | not started | store recovery |

## 8. Settings and providers

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| General | settings dialog | placeholder | QML form |
| Models (strong/cheap/vision) | `agent-section.tsx` | placeholder list | picker + persistence |
| Tool toggles / context | `agent-section.tsx` | not started | settings |
| Teaching style | `agent.teachingStyle` | placeholder toggle | Rust setting |
| Prompt overrides | prompts UI | not started | loader + editor |
| Appearance | `appearance.tsx` | dark shell only | tokens + controls |
| Graph settings | `appearance-section.tsx` | not started | render settings |
| Keyboard shortcuts | `keyboard-section.tsx` | not started | QML shortcuts |
| Keys | `keys-section.tsx` | not started | `auth.json` 0600 |
| Model catalog / pricing | `model-catalog.ts`, `prices.ts` | not started | port catalog JSON |
| OpenRouter + 40 providers | `pi-ai` catalogs | not started | provider trait |
| Free→paid fallback | `models.ts` | not started | payload fallback |
| Subscriptions (ChatGPT/Copilot/Grok) | `electron/*` | not started | OAuth/device code |
| Search engines | Tavily/Brave adapters | not started | HTTP + settings |

## 9. Platform

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Dictation | MediaRecorder + Deepgram | not started | cpal + WebSocket |
| PDF rendering | pdfjs-dist | not started | pdfium |
| Web research | search APIs | not started | reqwest |
| HTML/JS demos | sandboxed iframe | static plan only | lazy browser child |
| Visual verification | headless Chromium | not started | browser child |
| Themes | app appearance tokens | dark tokens only | light/dark |
| Data sync | external Syncthing | not started | format compatibility |
| Local data | `~/.config/LearnHarness` | scaffold only | same paths/format |

## 10. Tests and observability

| Feature | LearnHarness implementation | Brainroom status | Next |
|---|---|---|---|
| Unit/behavior tests | 189 files, 42.8k LOC | none | port per module |
| Faux provider | pi-ai faux core | none | Rust fake provider |
| Fixture parity | data fixtures | none | TS-generated fixtures |
| QML smoke | browser screenshots | screenshot harness working | QML tests |
| Event stream / activity | SSE + dock | none | QML events |
| Pricing/usage logging | `prices.ts`, turns log | none | usage ledger |

## Migration order

1. **Shell + UI parity** (current): window, home graph, session view, sidebar, settings shell.
2. **Core/store**: contracts, JSON/JSONL, settings, auth, model catalog, grading, plan tree.
3. **Agent loop**: OpenRouter streaming, tool calls, quiz/ask/plan, chat persistence.
4. **Graph**: real derivation, click/rename/delete, chain table, layout.
5. **Jobs/subagents**.
6. **Extraction**.
7. **Practice**.
8. **Providers/subscriptions**.
9. **Platform**.
10. **Hardening/parity**.

Each module lands with its tests and a `plan_check`/`plan_complete` gate; run `plan_audit` before moving on.
