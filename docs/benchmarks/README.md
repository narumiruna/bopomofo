# libchewing benchmark evidence

All reports retain `scope: fixture_only`. No report establishes product performance, Taiwan reading quality, native UI behavior or application compatibility. **Synthetic handle p95 is 8.20–8.31 ms, above 5 ms.** The representative product p95 < 5 ms gate remains open; this migration does not claim a speed improvement.

## Synthetic stress run

Use Rust 1.92.0, Git and uv from the repository root:

```bash
uv run --offline --no-project python -m unittest discover -s scripts/tests
uv run --offline --no-project python scripts/benchmark-synthetic.py
```

Nine lightweight generator tests run in CI. The full suite is a separate, long-running development check. It builds debug/release CLI binaries, generates three profiles, validates and exports each source twice, compiles assets twice, compares bytes, checks all expectations, compares debug/release/repeated replay output, and runs two release benchmarks with **100 repetitions each**. A failed command or mismatch stops the script. No native installation, real input capture or publication occurs.

### Retained reports

Generated TSV, JSONL, binary assets and raw reports remain in ignored `target/lexicon-stress/`. The small retained reports are copied here only after success:

| Entries | Generated TSV bytes | Exact readings | Filler readings | Replay events | Handle p95, two runs | Probe p95, two runs |
| --- | --- | --- | --- | --- | --- | --- |
| [10,000](synthetic-10000.json) | 390,268 | 69 | 38 | 331 | 8.252 / 8.203 ms | 1.423 / 1.342 µs |
| [100,000](synthetic-100000.json) | 3,981,521 | 421 | 390 | 331 | 8.226 / 8.247 ms | 1.452 / 1.402 µs |
| [250,000](synthetic-250000.json) | 9,970,799 | 1,007 | 976 | 332 | 8.311 / 8.274 ms | 1.382 / 1.262 µs |

RSS samples range from 7.15 to 12.89 MiB. They are not peak memory or allocation counts.

## Data and manual expectations

All synthetic entries and scores are self-authored, not copied from a dictionary or corpus. The data uses the upstream one-scalar-per-reading model:

- Sixteen primary entries have readings of 1–16 repeated `ㄧ`, text of 1–16 repeated `一`, and score 1,000,000.
- The one-syllable reading has 255 additional scalar candidates. Distinct decreasing scores fix their order without assuming an upstream equal-score tie rule.
- Fifteen other singleton readings allow the filler syllables to complete independently. Unknown syllables are not silently converted to raw composition.
- Filler entries have five readings, five scalars and at most 256 candidates per reading. Distinct scores fix each group's selection order.
- The replay tests 64 syllables, overflow rejection, all 256 candidates, index 255 on the tail page, editing, cancellation, one-time commit, and first/last filler groups.

Expectations are authored from the input contract, not copied from engine output. Every profile reaches 16 readings per phrase and 256 candidates per reading. All remain below input and event-count limits. This highly ambiguous distribution is a stress case, not a natural-language model.

## Default dictionary with a small synthetic corpus

[Default-data report](libchewing-default.json) uses the actual 160,440-entry upstream package and the retained [eight-event corpus](libchewing-default-corpus.jsonl). The public case composes `你好`, commits once, then checks a second Enter. It is **not a representative corpus**.

```bash
cargo run --release -p ime-cli --locked -- build-assets data/libchewing/source target/default-benchmark-assets
cargo run --release -p ime-cli --locked -- benchmark target/default-benchmark-assets docs/benchmarks/libchewing-default-corpus.jsonl 100
```

The two recorded runs have handle p95 0.0252 / 0.0240 ms. Memory asset validation takes 34.8 / 36.5 ms; `Engine::new` after shared assets takes 12.7 / 12.2 µs. These values do not measure full native context creation: C ABI constructors also validate and copy their supplied dictionary bytes. Default data and synthetic stress are reported separately.

## Identity and method

Reports retain asset/source/corpus/output hashes, both benchmark runs, generator/CLI hashes, source snapshot, actual Rust/Cargo versions, OS and CPU. Recorded host: Linux x86_64, WSL2 kernel 6.18.33.2, AMD Ryzen 9 7950X3D, Rust 1.92.0. The compiler field inside CLI output is unavailable; the wrapper records the actual compiler separately.

Source snapshots hash sorted relative `path + NUL + bytes + NUL`. The explicit set is `.gitattributes`, Cargo manifests/lock, the toolchain file, `crates/`, `platforms/`, `fixtures/`, `.github/`, `scripts/`, `data/cns11643/` and `data/libchewing/`. Ignored outputs and `docs/` are excluded. `base_commit` is HEAD before implementation commit; the snapshot identifies working-tree content, not signed provenance.

`per_key_latency` measures `Engine::handle` and its complete snapshot. `probe_latency` measures the read-only probe immediately before each key; it can warm caches. Timers exclude file I/O, event cloning, expectation checks, output serialization/destruction and per-repetition initialization. Every repetition starts with a new Engine; there is no separate warmup. The legacy `lexicon_parse_ns` field now means in-memory asset validation, not TSV parsing. The host was shared and had no CPU affinity or frequency control.

The older [fixture-64 report](fixture-64.json) is historical self-engine evidence, not a comparable product baseline. Pre-migration synthetic reports remain in Git history at `46da8ac`. Current checks and unresolved native/corpus/release gates are in the [validation register](../native-validation.md).
