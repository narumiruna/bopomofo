#!/usr/bin/env bash
# Rebuild a temporary custom profile. No native installation or desktop use.
set -euo pipefail
root=$1
work=$(mktemp -d /tmp/bopomofo-source-rebuild.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
source_file="$work/custom.tsv"
printf '# bopomofo-lexicon-v1\nㄧ\t一\t1\n' > "$source_file"
cmake -S "$root/platforms/linux/fcitx5" -B "$work/build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF "-DBOPOMOFO_ASSET_SOURCE=$source_file"
cmake --build "$work/build" --target bopomofo_abi_check --parallel 2
before=$(sha256sum "$work/build/assets/dictionary.dat")
printf '# bopomofo-lexicon-v1\nㄧ\t一\t1\nㄧ\t二\t2\n' > "$source_file"
cmake --build "$work/build" --target bopomofo_abi_check --parallel 2
after=$(sha256sum "$work/build/assets/dictionary.dat")
[[ "$before" != "$after" ]]
"$work/build/rust/release/ime-cli" validate-assets "$work/build/assets" | grep -F '"entries":2'
printf '%s\n' 'Custom authoring edit regenerated verified assets without a manual configure.'
