#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
LEXICON=${1:-"$ROOT/fixtures/lexicon.tsv"}
BUILD="$ROOT/platforms/macos/build"
APP="$BUILD/Bopomofo Development.app"
SDK=$(xcrun --sdk macosx --show-sdk-path)
ARCH=$(uname -m)
if [[ $# -gt 1 || ! -f "$LEXICON" ]]; then
    echo 'usage: build.sh [approved-lexicon.tsv]' >&2
    exit 1
fi
cd "$ROOT"
cargo build --release -p ime-ffi --target-dir "$ROOT/target"
mkdir -p "$BUILD"
xcrun --sdk macosx swiftc -sdk "$SDK" -target "$ARCH-apple-macosx13.0" -warnings-as-errors \
    -import-objc-header "$ROOT/crates/ime-ffi/include/bopomofo.h" \
    "$ROOT/platforms/macos/Sources/Adapter.swift" "$ROOT/platforms/macos/Sources/CoreEngine.swift" \
    "$ROOT/platforms/macos/Tests/ValidateLibrary.swift" "$ROOT/target/release/libime_ffi.a" \
    -o "$BUILD/validate-library"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/platforms/macos/Info.plist" "$APP/Contents/Info.plist"
cp "$LEXICON" "$APP/Contents/Resources/lexicon.tsv"
shasum -a 256 "$APP/Contents/Resources/lexicon.tsv" | awk '{print $1}' > "$APP/Contents/Resources/lexicon.sha256"
"$BUILD/validate-library" "$APP/Contents/Resources/lexicon.tsv" "$APP/Contents/Resources/lexicon.sha256"
xcrun --sdk macosx swiftc -sdk "$SDK" -target "$ARCH-apple-macosx13.0" \
    -module-name BopomofoDevelopment -O -warnings-as-errors \
    -import-objc-header "$ROOT/crates/ime-ffi/include/bopomofo.h" \
    "$ROOT"/platforms/macos/Sources/*.swift "$ROOT/target/release/libime_ffi.a" \
    -framework AppKit -framework InputMethodKit \
    -o "$APP/Contents/MacOS/BopomofoDevelopment"
plutil -lint "$APP/Contents/Info.plist"
/usr/bin/codesign --force --sign - --timestamp=none "$APP"
/usr/bin/codesign --verify --strict "$APP"
{
    printf 'Architecture: %s\nSDK: %s\nDeployment target: macOS 13.0 (not a support claim)\n' "$ARCH" "$SDK"
    xcrun --sdk macosx swiftc --version 2>&1
    shasum -a 256 "$APP/Contents/MacOS/BopomofoDevelopment" "$APP/Contents/Resources/lexicon.tsv" "$APP/Contents/Resources/lexicon.sha256"
} > "$BUILD/build-evidence.txt"
printf 'Bundle built (not installed or native-validated): %s\n' "$APP"
