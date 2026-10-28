#!/usr/bin/env bash
# Dry-run is the default. No recursive deletion and no profile changes.
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh" "$@"
for path in "${paths[@]}" .bopomofo-dev-owned; do
    printf 'REMOVE %q\n' "$prefix/$path"
done
$apply || exit 0
[[ -f "$prefix/.bopomofo-dev-owned" ]] || fail 'Missing installation ownership marker'
IFS= read -r marker < "$prefix/.bopomofo-dev-owned"
[[ $marker == bopomofo-fcitx5-dev-v1 ]] || fail 'Invalid installation ownership marker'
for path in "${paths[@]}"; do
    [[ ! -e "$prefix/$path" || -f "$prefix/$path" ]] || fail 'Expected a regular owned file'
done
for path in "${paths[@]}" .bopomofo-dev-owned; do
    rm -f -- "$prefix/$path"
done
# Empty directories are left in place. Unknown files are never removed.
