#!/usr/bin/env bash
# Synthetic input only. No IME installation, desktop connection or user input.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
command -v jq >/dev/null
command -v strace >/dev/null
work=$(mktemp -d /tmp/bopomofo-privacy-check.XXXXXX)
trap 'chmod u+w "$work/home" 2>/dev/null || true; rm -rf -- "$work"' EXIT
mkdir "$work/home"
chmod 555 "$work/home"
target=$(cargo metadata --locked --no-deps --format-version 1 | jq -er .target_directory)
mkdir -p "$target/libchewing-evidence"
for profile in debug release; do
    flags=()
    [[ $profile == debug ]] || flags=(--release)
    cargo test -p ime-core --test engine --locked --no-run --message-format=json "${flags[@]}" > "$work/build.jsonl"
    binary=$(jq -er 'select(.reason == "compiler-artifact" and .target.name == "engine" and .executable != null) | .executable' "$work/build.jsonl")
    trace="$target/libchewing-evidence/privacy-$profile.trace"
    (
        cd "$work/home"
        env HOME="$work/home" XDG_CONFIG_HOME="$work/home/config" XDG_DATA_HOME="$work/home/data" \
            CHEWING_PATH=/unavailable strace -f -e trace=%file,%network,read,write,pread64,pwrite64,readv,writev,preadv,pwritev,preadv2,pwritev2,close -o "$trace" \
            "$binary" --exact learning_and_logging_are_disabled --nocapture --test-threads=1
    ) > "$target/libchewing-evidence/privacy-$profile.log" 2>&1
    awk '
        /BOPOMOFO_IO_BEGIN/ { started++; active=1; next }
        /BOPOMOFO_IO_END/ { ended++; active=0; next }
        active { bad=1 }
        END { if (started!=1 || ended!=1 || active || bad) exit 1 }
    ' "$trace"
    [[ -z $(find "$work/home" -mindepth 1 -print -quit) ]]
    printf 'PASS: %s engine initialization, events, probe, selection, cancellation and drop have no traced file/network/read/write operations; HOME stays empty.\n' "$profile"
done
