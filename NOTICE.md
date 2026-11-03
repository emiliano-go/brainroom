# Attribution and notices

## LearnHarness

Brainroom is based on and inspired by **LearnHarness**:

- Project: <https://github.com/amosblomqvist/learnharness>
- Author: Amos Blomqvist
- License: GNU Affero General Public License v3.0 (AGPL-3.0)

LearnHarness is the reference for Brainroom's behavior, teaching methodology, prompt fragments, data formats, architecture, and design. Brainroom is an independent Rust + Qt implementation; it is **not affiliated with or endorsed by** the LearnHarness project or its author. The LearnHarness license text is included in [`LICENSE`](LICENSE).

If Brainroom later incorporates source code copied or adapted from LearnHarness, that code remains under AGPL-3.0 and this notice will identify the files.

## Corcho (implementation reference)

- Project: <https://github.com/emiliano-go/corcho>
- License: MIT
- Role: reference for Rust + Qt 6 + `cxx-qt` 0.10 application structure, QML modules, AI streaming, QtWebEngine integration, updates, single-instance handling, and AppImage/deb packaging.

Corcho is not vendored into Brainroom. If code is adapted from Corcho in the future, the MIT copyright notice will be added to the affected files and to this notice.

## Orchard (implementation reference)

- Project: <https://github.com/SFG5453/Orchard>
- License: AGPL-3.0-or-later
- Role: reference for a native Qt frontend with a Rust core over FFI, cross-platform packaging, provider/service boundaries, and Qt/Rust build orchestration.

Orchard is not vendored into Brainroom. If code is adapted from Orchard in the future, the AGPL notice and upstream attribution will be added to the affected files and to this notice.
