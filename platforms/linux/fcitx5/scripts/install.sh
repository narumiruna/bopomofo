#!/usr/bin/env bash
# Dry-run is the default. --apply needs separate user authorization.
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh" "$@"
sources=(libbopomofo-dev.so bopomofo-dev-addon.conf bopomofo-dev-inputmethod.conf lexicon.tsv lexicon.sha256)
for source in "${sources[@]}"; do
    [[ -f "$build/$source" && ! -L "$build/$source" ]] || fail 'Build all addon artifacts first'
done
for i in "${!paths[@]}"; do
    printf 'COPY %q -> %q\n' "$build/${sources[$i]}" "$prefix/${paths[$i]}"
done
$apply || exit 0
# Refuse every overwrite, including an earlier development install. Back up the
# existing prefix, then use the scoped uninstall before an authorized update.
for path in "${paths[@]}" .bopomofo-dev-owned; do
    [[ ! -e "$prefix/$path" ]] || fail 'Target exists; preserve it before updating'
done
for i in "${!paths[@]}"; do
    install -D -m 644 -- "$build/${sources[$i]}" "$prefix/${paths[$i]}"
done
printf 'bopomofo-fcitx5-dev-v1\n' > "$prefix/.bopomofo-dev-owned"
printf '%s\n' 'Files copied. No input method was enabled or registered in a profile.'
