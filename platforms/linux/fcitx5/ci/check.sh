#!/usr/bin/env bash
# Intended for an ephemeral container with /src mounted read-only.
set -euo pipefail
source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../../.." && pwd)
build=$(mktemp -d /tmp/bopomofo-fcitx5-build.XXXXXX)
trap 'rm -rf -- "$build"' EXIT
printf '%s\n' 'Environment (headless; no desktop checks):'
uname -sm
cmake --version | head -n 1
c++ --version | head -n 1
cargo --version
pkg-config --modversion Fcitx5Core xkbcommon nlohmann_json
cmake -S "$source_root/platforms/linux/fcitx5" -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "$build" --parallel 2
ctest --test-dir "$build" --output-on-failure
printf '%s\n' 'Artifact SHA-256 (this build only; no desktop validation):'
sha256sum "$build/libbopomofo-dev.so" "$build/rust/release/libime_ffi.a" \
    "$source_root/crates/ime-ffi/include/bopomofo.h" "$build/lexicon.tsv" "$build/lexicon.sha256"
for script in "$source_root/platforms/linux/fcitx5"/{scripts,ci}/*.sh; do bash -n "$script"; done
# No cmake --install, --apply, daemon, session, profile, or settings commands.
