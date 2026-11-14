#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
APP="$ROOT/platforms/macos/build/Bopomofo Development.app"
MANAGE="$ROOT/platforms/macos/scripts/manage.sh"
mkdir -p "$ROOT/platforms/macos/build"
SANDBOX=$(mktemp -d "$ROOT/platforms/macos/build/script-tests.XXXXXX")
trap 'rm -rf -- "$SANDBOX"' EXIT
MOCKHOME="$SANDBOX/home"
DEST="$MOCKHOME/Library/Input Methods/Bopomofo Development.app"
BACKUPS="$MOCKHOME/Library/Application Support/Bopomofo Development/Backups"
LOG="$SANDBOX/output.log"
TESTS=0
mkdir -p "$MOCKHOME"
fingerprint() {
    find "$MOCKHOME" -print | LC_ALL=C sort
    find "$MOCKHOME" -type f -exec shasum -a 256 {} \; | LC_ALL=C sort
}
run_ok() {
    local before
    before=$(fingerprint)
    HOME="$MOCKHOME" bash "$MANAGE" "$@" >>"$LOG" 2>&1
    [[ $(fingerprint) == "$before" ]] || { echo 'FAILED: dry-run changed files' >&2; exit 1; }
    TESTS=$((TESTS + 1))
}
run_fail() {
    local before
    before=$(fingerprint)
    if HOME="$MOCKHOME" bash "$MANAGE" "$@" >>"$LOG" 2>&1; then
        echo 'FAILED: unsafe request was accepted' >&2
        exit 1
    fi
    [[ $(fingerprint) == "$before" ]] || { echo 'FAILED: rejected request changed files' >&2; exit 1; }
    TESTS=$((TESTS + 1))
}
run_ok install
run_ok install --dry-run
run_fail install --invalid
run_fail restore ../unrelated.app --dry-run
mkdir -p "$(dirname "$DEST")"
ln -s "$APP" "$DEST"
run_fail install --dry-run
rm "$DEST"
mkdir -p "$DEST/Contents"
printf '%s\n' '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.unrelated.inputmethod</string></dict></plist>' > "$DEST/Contents/Info.plist"
run_fail install --dry-run
run_fail update --dry-run
run_fail uninstall --dry-run
rm -rf -- "$DEST"
/usr/bin/ditto "$APP" "$DEST"
run_ok update --dry-run
run_ok uninstall --dry-run
run_fail install --dry-run
mkdir -p "$BACKUPS"
/usr/bin/ditto "$APP" "$BACKUPS/20250101T000000Z-1.app"
run_ok restore 20250101T000000Z-1.app --dry-run
mv "$BACKUPS" "$SANDBOX/backups"
ln -s "$SANDBOX/backups" "$BACKUPS"
run_fail uninstall --dry-run
rm "$BACKUPS"
mv "$SANDBOX/backups" "$BACKUPS"
printf '\n' >> "$DEST/Contents/Resources/dictionary.dat"
run_fail uninstall --dry-run
/usr/bin/ditto "$APP" "$DEST"
printf '%064d\n' 0 > "$DEST/Contents/Resources/assets.json"
/usr/bin/codesign --force --sign - --timestamp=none "$DEST" >/dev/null 2>&1
run_fail uninstall --dry-run
/usr/bin/ditto "$APP" "$DEST"
rm "$DEST/Contents/Resources/assets.json"
/usr/bin/codesign --force --sign - --timestamp=none "$DEST" >/dev/null 2>&1
run_fail uninstall --dry-run
# Use a signed v1 resource layout. Do not run an old engine or install an IME.
LEGACY="$SANDBOX/legacy.app"
/usr/bin/ditto "$APP" "$LEGACY"
for name in dictionary.dat assets.json Engine-LICENSE.txt Engine-AUTHORS.txt Engine-NOTICE.txt Data-LICENSE.txt Data-NOTICE.txt; do
    rm "$LEGACY/Contents/Resources/$name"
done
cp "$ROOT/fixtures/lexicon.tsv" "$LEGACY/Contents/Resources/lexicon.tsv"
shasum -a 256 "$LEGACY/Contents/Resources/lexicon.tsv" | awk '{print $1}' > "$LEGACY/Contents/Resources/lexicon.sha256"
/usr/bin/codesign --force --sign - --timestamp=none "$LEGACY" >/dev/null 2>&1
reset_legacy_destination() {
    rm -rf -- "$DEST"
    /usr/bin/ditto "$LEGACY" "$DEST"
}
sign_destination() {
    /usr/bin/codesign --force --sign - --timestamp=none "$DEST" >/dev/null 2>&1
}

# A legacy package can be a retained bundle, but never a new install/update source.
LEGACY_PROJECT="$SANDBOX/legacy-source-project"
mkdir -p "$LEGACY_PROJECT/platforms/macos/scripts" "$LEGACY_PROJECT/platforms/macos/build"
cp "$MANAGE" "$LEGACY_PROJECT/platforms/macos/scripts/manage.sh"
/usr/bin/ditto "$LEGACY" "$LEGACY_PROJECT/platforms/macos/build/Bopomofo Development.app"
CURRENT_MANAGE=$MANAGE
MANAGE="$LEGACY_PROJECT/platforms/macos/scripts/manage.sh"
rm -rf -- "$DEST"
run_fail install --dry-run
reset_legacy_destination
run_fail update --dry-run
MANAGE=$CURRENT_MANAGE
run_ok update --dry-run
run_ok uninstall --dry-run

LEGACY_BACKUP=20250101T000000Z-2.app
/usr/bin/ditto "$LEGACY" "$BACKUPS/$LEGACY_BACKUP"
run_ok restore "$LEGACY_BACKUP" --dry-run
rm -rf -- "$DEST"
/usr/bin/ditto "$APP" "$DEST"
run_ok restore "$LEGACY_BACKUP" --dry-run
rm -rf -- "$DEST"
run_ok restore "$LEGACY_BACKUP" --dry-run

reset_legacy_destination
printf '%064d\n' 0 > "$DEST/Contents/Resources/lexicon.sha256"
sign_destination
run_fail uninstall --dry-run
reset_legacy_destination
tr -d '\n' < "$LEGACY/Contents/Resources/lexicon.sha256" > "$DEST/Contents/Resources/lexicon.sha256"
sign_destination
run_fail uninstall --dry-run
reset_legacy_destination
rm "$DEST/Contents/Resources/lexicon.sha256"
sign_destination
run_fail uninstall --dry-run
reset_legacy_destination
printf '\n' >> "$DEST/Contents/Info.plist"
run_fail uninstall --dry-run
reset_legacy_destination
cp "$APP/Contents/Resources/assets.json" "$DEST/Contents/Resources/assets.json"
sign_destination
run_fail uninstall --dry-run
printf '%064d\n' 0 > "$BACKUPS/$LEGACY_BACKUP/Contents/Resources/lexicon.sha256"
/usr/bin/codesign --force --sign - --timestamp=none "$BACKUPS/$LEGACY_BACKUP" >/dev/null 2>&1
run_fail restore "$LEGACY_BACKUP" --dry-run
printf 'Management scripts: %s dry-run safety cases passed (sandbox only; --apply never executed)\n' "$TESTS"
