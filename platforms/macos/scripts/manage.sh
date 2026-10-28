#!/bin/bash
# Only --apply permits writes. This script never selects or registers an input source.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd -P)
BUNDLE_ID=org.bopomofo.inputmethod.Development
SOURCE_ID=org.bopomofo.inputmethod.Development.zhuyin
CONNECTION=BopomofoDevelopment_Connection
NAME='Bopomofo Development.app'
SOURCE="$ROOT/platforms/macos/build/$NAME"

usage() {
    echo 'usage: manage.sh install|update|uninstall [--dry-run|--apply]'
    echo '       manage.sh restore <backup-name.app> [--dry-run|--apply]'
}
fail() { echo "error: $1" >&2; exit 1; }
ACTION=${1:-help}
case "$ACTION" in
    help|--help) usage; exit 0 ;;
    install|update|uninstall)
        [[ $# -le 2 ]] || { usage >&2; exit 1; }
        MODE=${2:---dry-run}
        ;;
    restore)
        [[ $# -ge 2 && $# -le 3 ]] || { usage >&2; exit 1; }
        BACKUP_NAME=$2
        [[ "$BACKUP_NAME" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9]+\.app$ ]] || fail 'invalid backup name'
        MODE=${3:---dry-run}
        ;;
    *) usage >&2; exit 1 ;;
esac
[[ "$MODE" == --apply || "$MODE" == --dry-run ]] || fail 'explicit mode must be --apply or --dry-run'
[[ "$HOME" == /* && "$HOME" != / && -d "$HOME" ]] || fail 'HOME must be an existing absolute current-user home'
[[ "$HOME" != */../* && "$HOME" != */.. && "$HOME" != */./* && "$HOME" != */. ]] || fail 'noncanonical HOME'
[[ $EUID -ne 0 ]] || fail 'do not run as root or with sudo'
INPUT_METHODS="$HOME/Library/Input Methods"
DEST="$INPUT_METHODS/$NAME"
BACKUPS="$HOME/Library/Application Support/Bopomofo Development/Backups"

# Reject symlink path components, including existing parents of paths not yet created.
check_path() {
    local path=$1
    while [[ "$path" != / ]]; do
        [[ ! -L "$path" ]] || fail 'symlink path refused'
        path=$(dirname "$path")
    done
}
check_path "$DEST"
check_path "$BACKUPS"
[[ $(stat -f '%u' "$HOME") == "$EUID" ]] || fail 'HOME is not owned by current user'

plist_value() { /usr/libexec/PlistBuddy -c "Print :$2" "$1/Contents/Info.plist" 2>/dev/null; }
validate_bundle() {
    local app=$1
    [[ -d "$app" && ! -L "$app" ]] || fail 'bundle is absent or a symlink'
    [[ -z $(find "$app" -type l -print -quit) ]] || fail 'bundle contains symlinks'
    plutil -lint "$app/Contents/Info.plist" >/dev/null 2>&1 || fail 'invalid bundle plist'
    [[ $(plist_value "$app" CFBundleIdentifier) == "$BUNDLE_ID" ]] || fail 'unrelated bundle identifier'
    [[ $(plist_value "$app" CFBundleExecutable) == BopomofoDevelopment ]] || fail 'unexpected executable'
    [[ $(plist_value "$app" TISInputSourceID) == "$SOURCE_ID" ]] || fail 'unrelated input source'
    [[ $(plist_value "$app" InputMethodConnectionName) == "$CONNECTION" ]] || fail 'unexpected connection'
    [[ $(plist_value "$app" InputMethodServerControllerClass) == BopomofoInputController ]] || fail 'unexpected controller'
    [[ -x "$app/Contents/MacOS/BopomofoDevelopment" && -f "$app/Contents/Resources/lexicon.tsv" &&
       -f "$app/Contents/Resources/lexicon.sha256" ]] || fail 'incomplete bundle'
    [[ $(wc -c < "$app/Contents/Resources/lexicon.sha256") -eq 65 ]] || fail 'invalid lexicon manifest'
    local actual expected
    actual=$(shasum -a 256 "$app/Contents/Resources/lexicon.tsv" | awk '{print $1}')
    expected=$(tr '[:upper:]' '[:lower:]' < "$app/Contents/Resources/lexicon.sha256")
    [[ "$expected" == "$actual" ]] || fail 'lexicon checksum mismatch'
    /usr/bin/codesign --verify --strict "$app" >/dev/null 2>&1 || fail 'invalid bundle signature'
}

case "$ACTION" in
    install)
        [[ ! -e "$DEST" ]] || fail 'destination exists; use update only for this project'
        validate_bundle "$SOURCE"
        ;;
    update)
        validate_bundle "$DEST"
        validate_bundle "$SOURCE"
        ;;
    uninstall) validate_bundle "$DEST" ;;
    restore)
        SOURCE="$BACKUPS/$BACKUP_NAME"
        check_path "$SOURCE"
        validate_bundle "$SOURCE"
        if [[ -e "$DEST" ]]; then validate_bundle "$DEST"; fi
        ;;
esac

printf 'Action: %s\nMode: %s\nDestination: %s\nBackup directory: %s\n' "$ACTION" "$MODE" "$DEST" "$BACKUPS"
if [[ "$ACTION" != uninstall ]]; then printf 'Source: %s\n' "$SOURCE"; fi
if [[ "$MODE" == --dry-run ]]; then
    echo 'No writes. No registration, input-source selection, process stop, or system settings change.'
    exit 0
fi

# The caller must first switch to a system input source and close active compositions.
mkdir -p "$INPUT_METHODS" "$BACKUPS"
BACKUP="$BACKUPS/$(date -u +%Y%m%dT%H%M%SZ)-$$.app"
[[ ! -e "$BACKUP" && ! -L "$BACKUP" ]] || fail 'backup path exists'
if [[ "$ACTION" == uninstall ]]; then
    mv "$DEST" "$BACKUP"
    printf 'Installation removed; retained backup: %s\n' "$BACKUP"
    exit 0
fi

STAGE=$(mktemp -d "$INPUT_METHODS/.bopomofo-development.XXXXXX")
cleanup() { rm -rf -- "$STAGE"; }
trap cleanup EXIT
/usr/bin/ditto "$SOURCE" "$STAGE/$NAME"
validate_bundle "$STAGE/$NAME"
HAD_PREVIOUS=false
if [[ -e "$DEST" ]]; then
    mv "$DEST" "$BACKUP"
    HAD_PREVIOUS=true
fi
if ! mv "$STAGE/$NAME" "$DEST"; then
    if [[ "$HAD_PREVIOUS" == true && ! -e "$DEST" ]]; then
        mv "$BACKUP" "$DEST" || fail 'automatic rollback failed; restore retained backup manually'
    fi
    fail 'installation failed; previous bundle restored if present'
fi
printf 'Installed: %s\n' "$DEST"
if [[ "$HAD_PREVIOUS" == true ]]; then printf 'Previous bundle: %s\n' "$BACKUP"; fi
echo 'No registration or input-source selection performed. Enable manually only with authorization.'
