# Prompt fragments

Copied from LearnHarness `src/agent/` (AGPL-3.0) so the Rune shell teaches with the
same contract. Composition order and override semantics mirror LearnHarness:

1. `system-prompt.md` for the teacher, then `teacher/*.md` sorted by filename.
2. `shared/*.md` as the trailing appendix for teacher/prober; practice composes
   only the allowlisted voice/accuracy/mental-model/formatting fragments.
3. A numeric-prefix `*.md` fragment only.
4. Override files mirror this tree under `<data>/prompts/`; the settings switch
   `agent.prompts[path]` decides whether the override replaces the source.
