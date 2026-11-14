#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
BUILD="$ROOT/platforms/macos/build"
SDK=$(xcrun --sdk macosx --show-sdk-path)
mkdir -p "$BUILD"
xcrun --sdk macosx swiftc -sdk "$SDK" -target "$(uname -m)-apple-macosx13.0" -warnings-as-errors \
    "$ROOT/platforms/macos/Sources/Adapter.swift" "$ROOT/platforms/macos/Sources/KeyTranslation.swift" \
    "$ROOT/platforms/macos/Sources/Presentation.swift" "$ROOT/platforms/macos/Tests/AdapterTests.swift" \
    -o "$BUILD/adapter-tests"
"$BUILD/adapter-tests" "$ROOT/platforms/macos/Tests/fixtures/native-keys.json"
bash "$ROOT/platforms/macos/scripts/build.sh"
STAGE=$(mktemp -d "$BUILD/test-assets.XXXXXX")
trap 'rm -rf -- "$STAGE"' EXIT
CLI="$ROOT/target/release/ime-cli"
"$CLI" build-assets "$ROOT/fixtures/lexicon.tsv" "$STAGE/fixture"
"$CLI" build-assets "$ROOT/data/cns11643/lexicon.tsv" "$STAGE/cns"
"$CLI" build-assets "$ROOT/fixtures/unicode/lexicon.tsv" "$STAGE/unicode"
xcrun --sdk macosx swiftc -sdk "$SDK" -target "$(uname -m)-apple-macosx13.0" -warnings-as-errors \
    -import-objc-header "$ROOT/crates/ime-ffi/include/bopomofo.h" \
    "$ROOT/platforms/macos/Sources/Adapter.swift" "$ROOT/platforms/macos/Sources/CoreEngine.swift" \
    "$ROOT/platforms/macos/Sources/KeyTranslation.swift" "$ROOT/platforms/macos/Sources/Presentation.swift" \
    "$ROOT/platforms/macos/Tests/FFITests.swift" "$ROOT/target/release/libime_ffi.a" \
    -o "$BUILD/ffi-tests"
"$BUILD/ffi-tests" "$STAGE/fixture/dictionary.dat" "$STAGE/fixture/assets.json" \
    "$STAGE/cns/dictionary.dat" "$STAGE/cns/assets.json" \
    "$STAGE/unicode/dictionary.dat" "$STAGE/unicode/assets.json"
for script in "$ROOT"/platforms/macos/scripts/*.sh; do bash -n "$script"; done
bash "$ROOT/platforms/macos/scripts/test-management.sh"
