#!/bin/bash
# Only --apply permits writes. This script never selects or registers an input source.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd -P)
BUNDLE_ID=org.bopomofo.inputmethod.Development
SOURCE_ID=org.bopomofo.inputmethod.Development.zhuyin
CONNECTION=BopomofoDevelopment_Connection
NAME='Bopomofo Development.app'
SOURCE="$ROOT/platforms/macos/build/$NAME"
SOURCE_KIND=current

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
    local app=$1 kind=${2:-current}
    [[ -d "$app" && ! -L "$app" ]] || fail 'bundle is absent or a symlink'
    [[ -z $(find "$app" -type l -print -quit) ]] || fail 'bundle contains symlinks'
    plutil -lint "$app/Contents/Info.plist" >/dev/null 2>&1 || fail 'invalid bundle plist'
    [[ $(plist_value "$app" CFBundleIdentifier) == "$BUNDLE_ID" ]] || fail 'unrelated bundle identifier'
    [[ $(plist_value "$app" CFBundleExecutable) == BopomofoDevelopment ]] || fail 'unexpected executable'
    [[ $(plist_value "$app" TISInputSourceID) == "$SOURCE_ID" ]] || fail 'unrelated input source'
    [[ $(plist_value "$app" InputMethodConnectionName) == "$CONNECTION" ]] || fail 'unexpected connection'
    [[ $(plist_value "$app" InputMethodServerControllerClass) == BopomofoInputController ]] || fail 'unexpected controller'
    local resources="$app/Contents/Resources" manifest="$app/Contents/Resources/assets.json"
    [[ -x "$app/Contents/MacOS/BopomofoDevelopment" ]] || fail 'incomplete bundle'
    if [[ "$kind" == retained && ! -e "$resources/dictionary.dat" && ! -e "$manifest" ]]; then
        # Only existing installations and rollback backups can use the v1 layout.
        local legacy="$resources/lexicon.tsv" checksum="$resources/lexicon.sha256"
        [[ -f "$legacy" && -f "$checksum" ]] || fail 'incomplete legacy bundle'
        [[ -s "$legacy" && $(wc -c < "$legacy") -le 67108864 &&
           $(wc -c < "$checksum") -eq 65 &&
           $(tail -c 1 "$checksum" | od -An -tu1) -eq 10 ]] || fail 'invalid legacy asset size'
        local actual expected
        actual=$(shasum -a 256 "$legacy" | awk '{print $1}')
        expected=$(tr '[:upper:]' '[:lower:]' < "$checksum")
        [[ "$expected" =~ ^[0-9a-f]{64}$ && "$expected" == "$actual" ]] || fail 'legacy checksum mismatch'
    else
        [[ -f "$resources/dictionary.dat" && -f "$manifest" ]] || fail 'incomplete bundle'
        [[ ! -e "$resources/lexicon.tsv" && ! -e "$resources/lexicon.sha256" ]] || fail 'mixed asset layout'
        [[ $(wc -c < "$manifest") -le 16384 && $(wc -c < "$resources/dictionary.dat") -le 33554432 ]] || fail 'asset size limit'
        [[ $(plutil -extract format raw -o - "$manifest") == bopomofo-assets-v1 &&
           $(plutil -extract engine raw -o - "$manifest") == chewing-0.13.1 &&
           $(plutil -extract builder raw -o - "$manifest") == bopomofo-assets-v1 &&
           $(plutil -extract files.0.name raw -o - "$manifest") == dictionary.dat ]] || fail 'incompatible asset manifest'
        if plutil -extract files.1 json -o - "$manifest" >/dev/null 2>&1; then fail 'unexpected asset' ; fi
        local actual expected length
        actual=$(shasum -a 256 "$resources/dictionary.dat" | awk '{print $1}')
        expected=$(plutil -extract files.0.sha256 raw -o - "$manifest")
        length=$(plutil -extract files.0.bytes raw -o - "$manifest")
        [[ "$expected" == "$actual" && "$length" -eq $(wc -c < "$resources/dictionary.dat") ]] || fail 'asset checksum mismatch'
        for name in Project-LICENSE.txt Engine-LICENSE.txt Engine-AUTHORS.txt Engine-NOTICE.txt Data-LICENSE.txt Data-NOTICE.txt; do
            [[ -s "$resources/$name" ]] || fail 'missing license or notice'
        done
    fi
    /usr/bin/codesign --verify --strict "$app" >/dev/null 2>&1 || fail 'invalid bundle signature'
}

case "$ACTION" in
    install)
        [[ ! -e "$DEST" ]] || fail 'destination exists; use update only for this project'
        validate_bundle "$SOURCE"
        ;;
    update)
        validate_bundle "$DEST" retained
        validate_bundle "$SOURCE"
        ;;
    uninstall) validate_bundle "$DEST" retained ;;
    restore)
        SOURCE="$BACKUPS/$BACKUP_NAME"
        SOURCE_KIND=retained
        check_path "$SOURCE"
        validate_bundle "$SOURCE" retained
        if [[ -e "$DEST" ]]; then validate_bundle "$DEST" retained; fi
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
validate_bundle "$STAGE/$NAME" "$SOURCE_KIND"
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
