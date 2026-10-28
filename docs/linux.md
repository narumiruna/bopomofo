# Linux Fcitx5 addon

## Status

The addon builds on **Linux aarch64, Ubuntu 24.04, Fcitx5 5.1.7** in an isolated Docker container. C++ ABI, shared replay, and headless Fcitx API contract tests pass. This is **not** a desktop integration result.

No IME was installed, enabled, or removed. No host settings were changed. All X11, Wayland, application, and x86_64 desktop combinations remain **unverified**. The fixture lexicon is not a production lexicon. See [native-validation.md](native-validation.md) for the project evidence register.

**Open P6 blocker:** Fcitx5 5.1.7 DBus `SelectCandidate(int)` has no render generation. A delayed client-side UI index is resolved against the current list. Captured `CandidateWord` guards do not protect this path. A headless model reproduces the gap; an end-to-end native delayed-click check has not run. P6 and native acceptance remain open.

## Adapter boundary

- `platforms/linux/fcitx5/src/addon.cpp` implements `fcitx::InputMethodEngine`.
  - A registered `FactoryFor<ContextProperty>` gives each input context its own Rust `Engine`. The property does not copy composition or mode to another context.
  - The addon reads the installed lexicon and its SHA-256 manifest once at creation. It verifies the checksum through the C ABI, validates the data, and keeps immutable source bytes. Missing, malformed, or mismatched data prevents addon creation. This checks pinned integrity, not authenticity. Each new context passes those bytes to Rust. Key handling does not open files.
- `src/keymap.cpp` loads a fixed US XKB map at addon creation.
  - Physical XKB keycodes select level 0 US printable characters. Actual native modifier/action symbols take priority. A US Caps/Control/action position does not override a remapped printable symbol. Fcitx XIM uses X11 keycodes. Its Wayland IM frontends add the evdev offset of 8 before creating a key event.
  - A code-less event has only a US keysym fallback. A non-US layout without physical keycodes is not verified.
  - Raw modifier flags preserve Ctrl, Alt/AltGr, Meta/Super/Hyper, Shift, repeat, and release. Control/Alt/Meta transitions and unknown keys also go to core. They can cancel a pending standalone-Shift gesture.
  - Native Shift state is context-local. The normalizer tracks both physical sides and delivers only the first aggregate press and final release to core. Other physical Shift transitions remain pass-through observations. This converts X11 pre-release masks without implementing gesture rules.
  - Composition reset, focus, and capability cancellation do not erase physical Shift state. Native release or a native no-Shift mask resynchronizes it. A second side pressed while another side remains held cannot rearm a gesture cancelled by Reset.
  - Shift-alone and Caps Lock mode changes are core rules. Caps Lock means the logical `caps_lock` key event, not the host LED state.
- Core alone parses Zhuyin, decodes text, moves candidates, pages, and selects digits.
  - The native candidate list displays the core page of at most 9 items. Its cursor is `candidate_index - page * 9`. Core indices are absolute.
  - Native page buttons send `page_up` or `page_down` to core. They do not change a local candidate page.
  - Selection through a captured `CandidateWord` sends its `generation` and absolute index to core. Core rejects a stale captured token. A context/engine lifetime guard also rejects callbacks after replacement or destruction. These checks do not supply the missing generation on the DBus index-only path.
  - Candidate lists appear only in core selection mode. The adapter does not intercept digits to implement a second selection rule.
- `Activate`, `Deactivate`, `FocusLost`, and `Reset` remain distinct events.
  - Fcitx calls `activate` on focus-in. A focus-out `deactivate` callback sends `focus_lost`; an input-method switch sends `deactivate`.
  - Preedit uses `TextFormatFlag::DontCommit`. This prevents Fcitx from committing client preedit before the focus-out callback. Only a transient core `commit` calls `commitString`, once.
  - A client with preedit capability receives client preedit. Otherwise, Fcitx displays panel preedit. Disabled, password, and sensitive contexts are not processed.
  - An `InputContextCapabilityChanged` watcher immediately sends `FocusLost` and clears an enabled context when it becomes restricted, even without another key or click. This covers Sensitive-only changes that do not make Fcitx switch the input method. Clearing a capability does not restore old composition or reactivate an inactive context.
- `src/core_client.cpp` checks the ABI version, return codes, buffer ownership, JSON types, UTF-8, byte cursor boundaries, candidate indices, spans, page bounds, modes, and status values.
  - Rust buffers use `bpm_buffer_free`; handles use `bpm_engine_free`. Outputs start as null/zero.
  - An ABI, output, or C++ failure discards the affected engine and clears native presentation. It does not retry a commit. Recovery occurs at the next activation.
  - The addon does not log input text, keys, or JSON errors. Do not enable Fcitx key tracing during private input.

### Fcitx cursor units and API sources

Fcitx `Text::cursor` and `Text::setCursor` use **UTF-8 bytes**, not Unicode scalar counts or UTF-16 units. Core already uses these units. The adapter validates a character boundary and the `int` range, then passes the byte offset unchanged. Contract tests include non-BMP Han, emoji, and a combining sequence.

Official Fcitx5 5.1.7 sources checked for this implementation:

- [text.h](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx/text.h): cursor byte units.
- [inputmethodengine.h](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx/inputmethodengine.h), [inputcontextproperty.h](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx/inputcontextproperty.h): callbacks and per-context factory.
- [candidatelist.h](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx/candidatelist.h): candidate and pageable interfaces.
- [instance.cpp](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx/instance.cpp), [textformatflags.h](https://github.com/fcitx/fcitx5/blob/5.1.7/src/lib/fcitx-utils/textformatflags.h): focus-out commit order and `DontCommit`.
- [xim.cpp](https://github.com/fcitx/fcitx5/blob/5.1.7/src/frontend/xim/xim.cpp), [waylandimserver.cpp](https://github.com/fcitx/fcitx5/blob/5.1.7/src/frontend/waylandim/waylandimserver.cpp), [waylandimserverv2.cpp](https://github.com/fcitx/fcitx5/blob/5.1.7/src/frontend/waylandim/waylandimserverv2.cpp): XKB keycode convention.
- [dbusfrontend.cpp, lines 367–374 and 458](https://github.com/fcitx/fcitx5/blob/5.1.7/src/frontend/dbusfrontend/dbusfrontend.cpp#L367-L374): current-list lookup and index-only `SelectCandidate` signature.

## Development dependencies

The tested Ubuntu 24.04 build image installs these packages:

```text
build-essential ca-certificates cmake curl ninja-build pkg-config
libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev
libxkbcommon-dev nlohmann-json3-dev
```

Rust 1.92.0 comes from rustup. The repository toolchain file also requests rustfmt and Clippy. C++17, CMake 3.22 or later, Fcitx5 Core 5.1.7 or later, xkbcommon 1.0 or later, and nlohmann_json 3.11 or later are build requirements. `xkb-data`, installed with these dependencies, provides the US map at addon creation.

The runtime needs matching Fcitx5 Core/Utils/Config shared libraries, xkbcommon, C++ runtime libraries, and XKB data. The Rust library is statically linked. A desktop check also needs the Fcitx5 daemon, UI, the session frontend, and the matching GTK or Qt input module where that route is tested. They are not desktop-tested by the build image.

### Native Linux build

Run from the repository root on the target Linux CPU. Install development dependencies only on a machine where this is authorized.

```bash
cmake -S platforms/linux/fcitx5 -B target/fcitx5 -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  "-DCMAKE_INSTALL_PREFIX=$HOME/.local/lib/bopomofo-fcitx5-dev"
cmake --build target/fcitx5 --parallel 2
ctest --test-dir target/fcitx5 --output-on-failure
```

CMake runs `cargo build --locked --release -p ime-ffi` into the build directory. It uses the C header from the same checkout. It rejects cross-compilation and runs an ABI version executable before it links the addon. The addon also compares `bpm_abi_version()` with `BPM_ABI_VERSION` at runtime.

Artifacts are `target/fcitx5/libbopomofo-dev.so`, the two generated `.conf` files, `lexicon.tsv`, and `lexicon.sha256`. The default source is `fixtures/lexicon.tsv`. To use different approved data, set `-DBOPOMOFO_LEXICON=/absolute/path/to/lexicon.tsv`. CMake copies it into the build directory and writes a manifest with 64 lowercase SHA-256 digits plus LF. The compiled runtime path is the configured prefix plus `share/fcitx5/bopomofo-dev/lexicon.tsv`; do not relocate an installed prefix without rebuilding.

If no prefix is supplied, CMake defaults to an isolated `bopomofo-fcitx5-dev` directory inside the build tree, not `/usr` or `/usr/local`. Build and CTest do not run an install target.

### Isolated container check

Docker availability was checked with `docker version`; the actual runner was OrbStack, Linux aarch64. These commands do not use a desktop or a writable source mount:

```bash
docker build -t bopomofo-fcitx5-check \
  -f platforms/linux/fcitx5/ci/Dockerfile platforms/linux/fcitx5/ci
docker run --rm \
  --mount "type=bind,source=$PWD,target=/src,readonly" \
  bopomofo-fcitx5-check
```

The base Ubuntu image is pinned by digest in `ci/Dockerfile`. The build downloads development tools and Cargo dependencies; input processing itself is offline. Outputs stay in the ephemeral container. `ci/check.sh` builds, runs CTest, and checks shell syntax. It does not run an IME daemon or installation command.

`ci/linux-headless.yml.in` is a preparation snippet for the common workflow owner. It requests separate Ubuntu x86_64 and ARM64 runners. It is not an active workflow and is not evidence that either CI job has run.

## Test evidence and limits

| Check | Actual result |
| --- | --- |
| Ubuntu 24.04 / Linux aarch64 / GCC 13.3 / CMake 3.28.3 / Rust 1.92.0 | Addon and contract executable built with warnings as errors. |
| Fcitx5 5.1.7 / xkbcommon 1.6.0 / nlohmann_json 3.11.3 | Linked into an ELF64 AArch64 addon with `--no-undefined`. Factory symbol exported; Rust ABI symbols remain private. |
| Build-time and CTest ABI version | Passed with header/library version 1. |
| C++ ABI/replay/headless Fcitx API contract | Passed. Includes all `fixtures/*.jsonl`, checksum tampering/missing manifest, 100 repeated independent handle pairs, invalid event/UTF-8/null input, read-only probe, Unicode byte cursor, one-shot commit, native candidate page/digit/click, captured stale selection/pager, lifecycle, reset while Shift remains held, Enter aliases, remapped Ctrl/Escape/Caps positions, and immediate Sensitive-only capability cancellation without a key. |
| C++ AddressSanitizer + UndefinedBehaviorSanitizer | Debug C++ build and CTest 4/4 passed with leak detection and halt-on-UB. Rust staticlib was a normal release build, not sanitizer-instrumented. |
| Install/uninstall dry-run and Bash syntax | Passed. No apply operation ran. |
| DBus delayed rendered-index selection | Known protocol gap reproduced by a controlled current-list lookup model. Not an end-to-end native test or safety pass; P6 remains blocked. |
| Linux x86_64 build | Unverified. Workflow preparation only. |
| Native addon load, X11/Wayland, toolkit/application integration | Unverified. No desktop session ran. |

The C++ contract executable accepts a lexicon path, its manifest path, and one or more shared JSONL replay files:

```bash
target/fcitx5/bopomofo_contract target/fcitx5/lexicon.tsv \
  target/fcitx5/lexicon.sha256 fixtures/*.jsonl
```

Each record supplies `event` and an `expect` subset of the core output. The runner compares expected fields; it does not regenerate expectations. Its mock input contexts call real Fcitx library APIs. A capability test calls the exact registered watcher handler directly because the standalone manager has no Instance event bus. These tests do not exercise a frontend, compositor, daemon addon loader, or application. Core tests, this platform build, and native desktop checks are separate evidence classes.

### Known DBus selection gap

The contract suite separately models Fcitx5 5.1.7 `selectCandidate(int)`. It captures a page-relative rendered index, resets and replaces the list, then resolves that old index against the current list as the DBus frontend does. The selected current `CandidateWord` has a fresh core generation, so core cannot identify the old UI action. A stale captured old `CandidateWord` is rejected in the same model; these are different paths.

The model only reproduces a known limitation. It does not validate a DBus client, delayed delivery, desktop frontend, or application. End-to-end stale client-side UI safety needs a framework/client generation contract or an explicitly approved design change. This addon does not change the Fcitx protocol, disable native clicks, or add a custom candidate window. Keep the P6/native gate open.

### C++ sanitizer check

The additional C++ sanitizer check used these flags in the same Linux container:

```bash
cmake -S platforms/linux/fcitx5 -B target/fcitx5-sanitized -G Ninja \
  -DCMAKE_BUILD_TYPE=Debug \
  '-DCMAKE_CXX_FLAGS=-fsanitize=address,undefined -fno-omit-frame-pointer' \
  '-DCMAKE_EXE_LINKER_FLAGS=-fsanitize=address,undefined' \
  '-DCMAKE_SHARED_LINKER_FLAGS=-fsanitize=address,undefined'
cmake --build target/fcitx5-sanitized --parallel 2
ASAN_OPTIONS=detect_leaks=1 UBSAN_OPTIONS=halt_on_error=1 \
  ctest --test-dir target/fcitx5-sanitized --output-on-failure
```

## Scoped install, update, and removal

Actual native installation, activation, removal, and settings changes need explicit user authorization. The following default commands only show paths:

```bash
bash platforms/linux/fcitx5/scripts/install.sh target/fcitx5
bash platforms/linux/fcitx5/scripts/uninstall.sh target/fcitx5
```

Only an authorized operator may append `--apply`. The scripts refuse root, symlinks, a prefix outside `HOME`, or a prefix whose final component is not `bopomofo-fcitx5-dev`. Installation refuses overwrites. Removal needs the project ownership marker and deletes only these five files and that marker:

```text
lib/fcitx5/libbopomofo-dev.so
share/fcitx5/addon/bopomofo-dev.conf
share/fcitx5/inputmethod/bopomofo-dev.conf
share/fcitx5/bopomofo-dev/lexicon.tsv
share/fcitx5/bopomofo-dev/lexicon.sha256
.bopomofo-dev-owned
```

The prefix is deliberately not a system addon directory. These scripts do not edit a Fcitx profile, enable an input method, set toolkit environment variables, or restart Fcitx. To test this prefix, an authorized operator must make its `lib/fcitx5` and `share/fcitx5` visible to a separate test Fcitx process using Fcitx's `FCITX_ADDON_DIRS` and `FCITX_DATA_DIRS`, retaining the distribution paths. Do not change the current desktop session without authorization.

For an update, first switch to another input method and stop the test process. Preserve a copy of the old isolated prefix and its matching build before the scoped removal. Then install the new build at the same compiled prefix. To roll back, stop the test process, remove the new build with the scoped script, and restore the preserved prefix at that exact path. Keep unknown files and the normal Fcitx profile unchanged. If a copy fails partway, stop and inspect the listed paths; do not run a broad recursive deletion.

## Native validation handoff

### Required matrix

Record each CPU separately: **aarch64** and **x86_64**. Start with Ubuntu 24.04 and Fcitx5 5.1.7. For each row, record the exact OS image, CPU, compositor/window manager, Fcitx version, application version, toolkit version, input route, and launch flags. All rows below are currently **unverified**; none is an advertised support combination.

| Session | Client under test | Explicit route / separate result required |
| --- | --- | --- |
| X11 | GTK3: Mousepad | X11 GTK Fcitx input module. |
| X11 | GTK4: GNOME Text Editor | X11 GTK Fcitx input module. |
| X11 | Qt5: KWrite | XCB platform and Qt5 Fcitx input module. |
| X11 | Qt6: Qt Creator editor | XCB platform and Qt6 Fcitx input module. |
| X11 | Firefox | Native X11 window; verify GTK input route. |
| X11 | Chromium | Ozone X11; record the actual input route and flags. |
| X11 | Electron: Visual Studio Code | Ozone X11; record Electron version and flags. |
| Wayland | GTK3: Mousepad | Native Wayland window and GTK Fcitx input module. |
| Wayland | GTK4: GNOME Text Editor | Native Wayland window and GTK Fcitx input module. |
| Wayland | Qt5: KWrite | Wayland platform and Qt5 Fcitx input module. |
| Wayland | Qt6: Qt Creator editor | Wayland platform and Qt6 Fcitx input module. |
| Wayland | Firefox | Native Wayland window; record GTK module or compositor text-input route. Test each used route separately. |
| Wayland | Chromium | Native Ozone Wayland with supported Wayland IME flags; record text-input protocol and compositor. |
| Wayland | Electron: Visual Studio Code | Native Ozone Wayland; record Electron IME flags, text-input protocol, and compositor. |
| Wayland session / XWayland client | Each GTK, Qt, browser, and Electron client above | Run each as an X11 window. Results do not validate its native Wayland row. |

A Wayland result is specific to its compositor and protocol. A GTK module result does not validate compositor text-input. A browser result does not validate Electron. A Qt5 result does not validate Qt6. Verify the actual window backend; a Wayland session alone is not evidence of a native Wayland client.

### Checks in every matrix cell

Use only public, self-authored fixture input. Do not capture private keystrokes, preedit, or committed text in diagnostics.

1. Load the addon through Fcitx's native loader. Select **Bopomofo Development** in the isolated test profile. Record build identity and library versions.
2. Type `1 8 Space`: show `八` as preedit, with a byte-correct end cursor. Type Enter: commit once and clear preedit. Repeat Enter: no second commit.
3. Type `1 8 Space Down`: show candidates 1–9. Page Down: show candidates 10–12 with page-relative labels. Press `1`: select `豝`, without commit. Enter commits it once. Repeat with native mouse selection and native page buttons.
4. Type `s u 3 c l 3`: show `你好`. Use Home, Right, Backspace, and Delete. Verify caret, native rendering, and the same core replay results. Use a separately approved self-authored Unicode lexicon to check non-BMP Han, emoji, and combining marks.
5. Hold a key: verify repeat. Release it: no extra syllable or commit. Test Ctrl/Alt/Meta shortcuts, shifted letters and punctuation, Shift-alone, Shift plus another key, Shift plus Control transition, and Caps Lock. Check the physical US Dachen positions under a non-US host layout.
6. Switch fields, windows, applications, and input methods during partial and complete composition. Expect cancellation with no commit and no state transfer. Repeat reset, including while one or both Shift keys remain physically held. Reopen candidates and test captured old callbacks separately from a delayed DBus `SelectCandidate(index)` from the old rendered UI. The DBus path remains an open protocol gap and blocks native acceptance.
7. Check password, sensitive, and IME-disabled fields. Change an existing field to Sensitive-only while preedit and candidates are visible, without pressing another key. Presentation must clear immediately with no commit. Clear Sensitive and check that Enter and old candidate callbacks cannot restore the old composition. No input UI must appear in a restricted field. Do not override the host restriction.
8. Check candidate position near each screen edge, scale changes, and multiple monitors. Verify both client preedit and the panel fallback where the client supports each mode.
9. Switch away, stop the test process, and use the authorized scoped removal. Verify that only project-owned paths were removed and that the normal input method still works. Restore the preserved version to test rollback.

Report passed, failed, blocked, and not-run checks separately. Attach non-sensitive build/test output and explicit observations for each matrix cell. Do not replace missing native results with container, replay, or cross-build results.
