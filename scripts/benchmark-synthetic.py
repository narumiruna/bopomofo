#!/usr/bin/env python3
"""Generate and measure self-authored synthetic lexicons, never product data."""

from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target/lexicon-stress"
PROFILES = (10_000, 100_000, 250_000)
REPETITIONS = 100
HEADER = "# bopomofo-lexicon-v1\n"
MAX_BYTES = 32 * 1024 * 1024
SEED_ENTRIES = 256 + 15
SYLLABLES = (
    "ㄧ", "ㄨ", "ㄩ", "ㄚ", "ㄛ", "ㄜ", "ㄞ", "ㄟ",
    "ㄠ", "ㄡ", "ㄢ", "ㄣ", "ㄤ", "ㄥ", "ㄦ", "ㄇㄚ",
)
KEYS = (
    "u", "j", "m", "8", "i", "k", "9", "o",
    "l", ".", "0", "p", ";", "/", "-", "a8",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def filler_reading(group):
    """All filler groups share two syllables; the last three encode the group."""
    if not 0 <= group < 16**3:
        raise ValueError("synthetic reading group out of range")
    suffix = (SYLLABLES[(group // 16**power) % 16] for power in (2, 1, 0))
    return ("ㄨ", "ㄩ", *suffix)


def lexicon_bytes(entries):
    if not SEED_ENTRIES <= entries <= PROFILES[-1]:
        raise ValueError("synthetic entry count out of range")
    records = ["ㄧ\t詞01\t1000000\n"]
    records.extend(f"ㄧ\t候選{index:03d}\t1\n" for index in range(1, 256))
    records.extend(
        f"{' '.join(['ㄧ'] * length)}\t詞{length:02d}\t1000000\n"
        for length in range(2, 17)
    )
    for index in range(entries - SEED_ENTRIES):
        reading = " ".join(filler_reading(index // 256))
        records.append(f"{reading}\t合成{index:06d}\t1\n")
    data = (HEADER + "".join(records)).encode("utf-8")
    if len(data) > MAX_BYTES:
        raise ValueError("synthetic lexicon exceeds CLI byte limit")
    return data


def decoded_prefix(count):
    """Equal primary scores prefer fewest segments, then longest left span."""
    full, remainder = divmod(count, 16)
    return "詞16" * full + (f"詞{remainder:02d}" if remainder else "")


def replay_bytes(entries):
    """Expectations follow input-spec and decoder rules, not engine output."""
    if not SEED_ENTRIES <= entries <= PROFILES[-1]:
        raise ValueError("synthetic entry count out of range")
    records = []

    def key(name, snapshot=None, commit=None, consumed=True, status=None):
        expected = {"consumed": consumed, "commit": commit, "status": status}
        if snapshot is not None:
            expected["snapshot"] = snapshot
        records.append({"event": {"type": "key", "key": name}, "expect": expected})

    def compose(count):
        for length in range(1, count + 1):
            key("u")
            text = decoded_prefix(length)
            key("space", {"preedit": text, "cursor": len(text.encode("utf-8"))})

    compose(64)
    text64 = decoded_prefix(64)
    key("u", {"preedit": text64}, status="limit_reached")
    candidates = [{"text": "詞01", "start": 63, "end": 64}]
    candidates.extend(
        {"text": f"候選{index:03d}", "start": 63, "end": 64}
        for index in range(1, 256)
    )
    key("down", {"preedit": text64, "selecting": True, "candidates": candidates})
    key("page_down", {"page": 1, "candidate_index": 9})
    selected = decoded_prefix(63) + "候選010"
    key("2", {
        "preedit": selected,
        "selecting": False,
        "cursor": len(selected.encode("utf-8")),
    })
    key("down", {"preedit": selected, "selecting": True, "candidates": candidates})
    for page in range(1, 29):
        key("page_down", {"page": page, "candidate_index": page * 9})
    selected_tail = decoded_prefix(63) + "候選255"
    key("4", {
        "preedit": selected_tail,
        "selecting": False,
        "cursor": len(selected_tail.encode("utf-8")),
    })
    key("backspace", {"preedit": decoded_prefix(63)})
    key("home", {"cursor": 0})
    key("delete", {"preedit": decoded_prefix(62), "cursor": 0})
    key("u")
    key("space", {"preedit": decoded_prefix(63), "cursor": 0})
    key("end")
    key("u")
    key("space", {"preedit": text64})
    key("escape", {"preedit": "", "cursor": 0, "candidates": [], "selecting": False})
    compose(64)
    key("enter", {"preedit": "", "candidates": []}, commit=text64)
    key("enter", {"preedit": ""}, consumed=False)

    # Exercise the first and last populated common-prefix index groups as well.
    remaining = entries - SEED_ENTRIES
    if remaining:
        for group in dict.fromkeys((0, (remaining - 1) // 256)):
            for syllable in filler_reading(group):
                for character in KEYS[SYLLABLES.index(syllable)]:
                    key(character)
                # ㄨ/ㄩ and other filler singletons are intentionally unknown.
                status = None if syllable == "ㄧ" else "unknown_reading"
                key("space", status=status)
            text = f"合成{group * 256:06d}"
            key("enter", {"preedit": ""}, commit=text)
            key("enter", consumed=False)

    if len(records) * REPETITIONS > 1_000_000:
        raise ValueError("synthetic replay exceeds benchmark event limit")
    return b"".join(
        (json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        for record in records
    )


def run(*args):
    return subprocess.run(args, cwd=ROOT, check=True, stdout=subprocess.PIPE).stdout


def source_identity():
    """Hash source/build inputs, not reports, Git metadata, or local outputs."""
    paths = run(
        "git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"
    ).split(b"\0")
    names = sorted(set(path.decode("utf-8") for path in paths if path))
    names = [
        name for name in names
        if name in (".gitattributes", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml")
        or name.startswith((
            "crates/", "platforms/", "fixtures/", ".github/", "scripts/", "data/cns11643/",
        ))
    ]
    checksum = hashlib.sha256()
    for name in names:
        checksum.update(name.encode("utf-8") + b"\0" + (ROOT / name).read_bytes() + b"\0")
    return {
        "base_commit": run("git", "rev-parse", "HEAD").decode().strip(),
        "snapshot_sha256": checksum.hexdigest(),
        "paths": len(names),
        "method": "sorted path + NUL + bytes + NUL; source/build/fixture/scripts/CNS inputs only",
    }


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    run("cargo", "build", "-p", "ime-cli", "--locked")
    run("cargo", "build", "-p", "ime-cli", "--release", "--locked")
    debug = ROOT / "target/debug/ime-cli"
    release = ROOT / "target/release/ime-cli"
    identity = source_identity()
    provenance = {
        "source": identity,
        "generator_sha256": digest(Path(__file__).read_bytes()),
        "cli_sha256": digest(release.read_bytes()),
        "rustc_verbose": run("rustc", "--version", "--verbose").decode().strip(),
        "cargo_version": run("cargo", "--version").decode().strip(),
    }
    for entries in PROFILES:
        directory = OUTPUT / str(entries)
        directory.mkdir(exist_ok=True)
        lexicon = directory / "synthetic.tsv"
        corpus = directory / "synthetic.jsonl"
        data, replay = lexicon_bytes(entries), replay_bytes(entries)
        lexicon.write_bytes(data)
        corpus.write_bytes(replay)
        if data != lexicon_bytes(entries) or replay != replay_bytes(entries):
            raise ValueError("synthetic generation is not deterministic")
        validation = json.loads(run(str(release), "validate-lexicon", str(lexicon)))
        canonical = directory / "canonical.tsv"
        second = directory / "canonical-repeat.tsv"
        built = json.loads(run(str(release), "build-lexicon", str(lexicon), str(canonical)))
        run(str(release), "build-lexicon", str(lexicon), str(second))
        if canonical.read_bytes() != second.read_bytes():
            raise ValueError("canonical lexicon rebuild differs")
        run(str(release), "validate-lexicon", str(canonical))
        outputs = [
            run(str(binary), "replay", str(canonical), str(corpus))
            for binary in (debug, release, release)
        ]
        if outputs[0] != outputs[1] or outputs[1] != outputs[2]:
            raise ValueError("debug/release/repeated replay differs")
        reports = [
            json.loads(run(
                str(release), "benchmark", str(canonical), str(corpus), str(REPETITIONS)
            ))
            for _ in range(2)
        ]
        for report in reports:
            if (
                report["scope"] != "fixture_only"
                or report["lexicon"]["entries"] != entries
                or report["lexicon"]["sha256"] != built["sha256"]
                or report["corpus"]["sha256"] != digest(replay)
                or report["corpus"]["total_events"] > 1_000_000
            ):
                raise ValueError("benchmark identity or scope differs")
        readings = Counter(
            line.split("\t")[0] for line in data.decode("utf-8").splitlines()[1:]
        )
        distribution = {
            "readings": len(readings),
            "max_candidates_per_reading": max(readings.values()),
            "max_phrase_syllables": max(len(reading.split(" ")) for reading in readings),
            "common_prefix_filler_readings": sum(reading.startswith("ㄨ ㄩ ") for reading in readings),
        }
        result = {
            "data_kind": "synthetic",
            "scores": "self-authored ranking values, not observed frequency",
            **provenance,
            "generated": validation,
            "canonical": built,
            "distribution": distribution,
            "replay_output_sha256": digest(outputs[0]),
            "checks": {
                "deterministic_generation": True,
                "canonical_rebuild_identical": True,
                "debug_release_replay_identical": True,
                "repeated_replay_identical": True,
            },
            "runs": reports,
        }
        destination = OUTPUT / f"synthetic-{entries}.json"
        destination.write_text(
            json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        print(f"synthetic entries={entries} bytes={len(data)} report={destination.relative_to(ROOT)}", flush=True)


if __name__ == "__main__":
    main()
