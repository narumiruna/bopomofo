#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
SOURCE=${1:-"$ROOT/data/libchewing/source"}
BUILD="$ROOT/platforms/macos/build"
APP="$BUILD/Bopomofo Development.app"
SDK=$(xcrun --sdk macosx --show-sdk-path)
ARCH=$(uname -m)
if [[ $# -gt 1 || ! -e "$SOURCE" ]]; then
    echo 'usage: build.sh [approved-authoring.tsv|upstream-source-directory]' >&2
    exit 1
fi
cd "$ROOT"
cargo build --locked --release -p ime-ffi -p ime-cli --target-dir "$ROOT/target"
mkdir -p "$BUILD"
STAGE=$(mktemp -d "$BUILD/assets-stage.XXXXXX")
trap 'rm -rf -- "$STAGE"' EXIT
"$ROOT/target/release/ime-cli" build-assets "$SOURCE" "$STAGE/assets"
xcrun --sdk macosx swiftc -sdk "$SDK" -target "$ARCH-apple-macosx13.0" -warnings-as-errors \
    -import-objc-header "$ROOT/crates/ime-ffi/include/bopomofo.h" \
    "$ROOT/platforms/macos/Sources/Adapter.swift" "$ROOT/platforms/macos/Sources/CoreEngine.swift" \
    "$ROOT/platforms/macos/Tests/ValidateLibrary.swift" "$ROOT/target/release/libime_ffi.a" \
    -o "$BUILD/validate-library"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/platforms/macos/Info.plist" "$APP/Contents/Info.plist"
# Remove only obsolete, generated v1 resources from this build bundle.
rm -f "$APP/Contents/Resources/lexicon.tsv" "$APP/Contents/Resources/lexicon.sha256" \
    "$APP/Contents/Resources/Lexicon-NOTICE.txt" "$APP/Contents/Resources/Lexicon-LICENSE.txt"
cp "$STAGE/assets/"* "$APP/Contents/Resources/"
"$BUILD/validate-library" "$APP/Contents/Resources/dictionary.dat" "$APP/Contents/Resources/assets.json"
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
    printf 'Architecture: %s\nSDK: %s\nABI: 2\nDeployment target: macOS 13.0 (not a support claim)\n' "$ARCH" "$SDK"
    xcrun --sdk macosx swiftc --version 2>&1
    shasum -a 256 "$APP/Contents/MacOS/BopomofoDevelopment" "$APP/Contents/Resources/dictionary.dat" "$APP/Contents/Resources/assets.json"
} > "$BUILD/build-evidence.txt"
printf 'Bundle built (not installed or native-validated): %s\n' "$APP"
