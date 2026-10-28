#!/usr/bin/env bash
# Shared path checks. No settings changes and no Fcitx process control.
set -euo pipefail
fail() { printf '%s\n' "$1" >&2; exit 1; }
[[ $# -ge 1 && $# -le 2 ]] || fail 'Usage: script BUILD_DIR [--apply]'
apply=false
if [[ $# == 2 ]]; then
    [[ $2 == --apply ]] || fail 'Only --apply is accepted'
    apply=true
fi
build=$(realpath -e -- "$1")
[[ -f "$build/prefix.txt" ]] || fail 'Missing configured prefix.txt'
IFS= read -r prefix < "$build/prefix.txt"
[[ $prefix == /*/bopomofo-fcitx5-dev ]] || fail 'Prefix must end in /bopomofo-fcitx5-dev'
[[ $(realpath -m -- "$prefix") == "$prefix" ]] || fail 'Prefix must be canonical without symlinks'
# Refuse symbolic links anywhere in the target tree. A symlink must not redirect
# one of the owned paths into a different input method or a system path.
check_path() {
    local current=$1
    while [[ $current != / ]]; do
        [[ ! -L $current ]] || fail 'Symbolic target paths are not allowed'
        current=$(dirname -- "$current")
    done
}
paths=(
    lib/fcitx5/libbopomofo-dev.so
    share/fcitx5/addon/bopomofo-dev.conf
    share/fcitx5/inputmethod/bopomofo-dev.conf
    share/fcitx5/bopomofo-dev/lexicon.tsv
    share/fcitx5/bopomofo-dev/lexicon.sha256
)
for path in "${paths[@]}"; do check_path "$prefix/$path"; done
check_path "$prefix/.bopomofo-dev-owned"
if $apply; then
    [[ $EUID -ne 0 ]] || fail 'Do not install or uninstall as root'
    home=$(realpath -e -- "$HOME")
    [[ $prefix == "$home"/* ]] || fail 'Apply is restricted to a prefix below HOME'
fi
