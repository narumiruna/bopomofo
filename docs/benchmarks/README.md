# Synthetic lexicon stress checks

These reports measure self-authored synthetic data only. Every CLI report retains `scope: fixture_only`. They do not establish Taiwan reading quality, candidate quality, native UI behavior, or representative product performance. The production-lexicon/corpus p95 < 5 ms gate remains blocked.

## Run

From the repository root, with Rust 1.92.0, uv, Git, and a local Python interpreter:

```bash
uv run --no-project python scripts/benchmark-synthetic.py
```

The script uses the Python standard library. It builds the existing debug and release CLI, generates three fixed profiles, validates each lexicon, builds it twice, and compares canonical bytes. It checks every replay expectation, compares debug/release/repeated replay output, then runs the existing release benchmark twice with 100 repetitions per run. A CLI failure or mismatch stops the script with a nonzero exit status.

Generated TSV, JSONL, canonical copies, and reports stay in ignored `target/lexicon-stress/`. The script does not read an installed IME, capture keyboard input, import external data, change runtime policy, or publish artifacts. Run it on a development host with sufficient disk and memory; the recorded host is macOS arm64. No Windows script execution is claimed.

The lightweight offline tests do not build Cargo or run the stress benchmark:

```bash
uv run --no-project python -m unittest discover -s scripts/tests
```

The existing Linux CI job runs these nine generator tests with pinned uv 0.9.26. Full profiles are not a CI step. Rust tests separately accept 250,000 entries, reject entry 250,001, accept 256 candidates per reading, and reject candidate 257. CLI tests also verify that candidate-limit failure preserves the old canonical file and removes staging files.

## Data and independent expectations

Scores are artificial ranking values, not observed frequency. No entry comes from a dictionary or corpus.

- Each profile has 16 primary entries: repeated `ㄧ` readings of lengths 1 through 16, with score 1,000,000 and text `詞01` through `詞16`.
- The single-syllable reading has 255 more candidates, `候選001` through `候選255`, each with score 1. Together with `詞01`, it reaches the 256-candidate lexicon limit.
- Filler readings start with `ㄨ ㄩ`, followed by three base-16 syllables. Each group has at most 256 unique synthetic texts and score 1. The final group can be partial.
- The replay composes 64 syllables, attempts an overflow, pages a 256-item snapshot, selects an alternative and the last candidate at index 255, deletes and inserts syllables, cancels, and tests one-time commit. It also exercises the first and last filler groups.

Expectations are authored from the input specification and decoder rules, not copied from engine output. Equal primary scores prefer fewer segments, then the longest leftmost span. Thus 64 repeated syllables yield `詞16詞16詞16詞16`; 63 yield `詞16詞16詞16詞15`. At the last syllable, page two and key `2` select `候選010`, locking only that syllable. A new selection starts at page zero; 28 Page Down events reach index 252 on page 28. Key `4` selects the final `候選255`, not a truncated 128-item list. Backspace removes the lock and restores the 63-syllable result. Unknown filler singletons preserve their reading until the complete five-syllable phrase matches.

| Entries | TSV bytes | Exact readings | Shared-prefix filler readings | Replay events | Events per 100 repetitions |
| --- | --- | --- | --- | --- | --- |
| 10,000 | 346,921 | 55 | 39 | 325 | 32,500 |
| 100,000 | 3,526,105 | 406 | 390 | 325 | 32,500 |
| 250,000 | 8,828,716 | 992 | 976 | 326 | 32,600 |

Every profile reaches 16 syllables per phrase and 256 candidates per reading. All remain below the CLI 32 MiB input limit and the benchmark 1,000,000-event limit. This concentrated distribution stresses known limits; it is not a model of natural language or typical input.

## Retained reports

- [10,000 entries](synthetic-10000.json)
- [100,000 entries](synthetic-100000.json)
- [250,000 entries](synthetic-250000.json)

Each small report retains both benchmark runs, generated/canonical lexicon hashes, corpus and replay-output hashes, actual distribution, source snapshot, generator and release-CLI hashes, actual `rustc --version --verbose`, Cargo version, OS, and CPU. The CLI's embedded compiler field remains unavailable; the wrapper records the actual compiler used for the build instead of changing runtime reporting.

The source snapshot uses sorted repository-relative paths, then `path + NUL + contents + NUL`, hashed with SHA-256. Inputs are `.gitattributes`, Cargo manifests/lock, the toolchain file, `crates/`, `platforms/`, `fixtures/`, `.github/`, `scripts/`, and `data/cns11643/`; ignored outputs and `docs/` are excluded. Fixture README files remain in the hashed `fixtures/` inputs. `base_commit` identifies HEAD before the source changes were committed. The snapshot identifies the tested content; it is not a signed provenance claim. The CNS files identify the rebased CLI test inputs, not the timed lexicon: all three benchmark lexicons remain synthetic.

Each run records lexicon initialization, nearest-rank p50/p95/p99 key latency, and a process RSS sample after the benchmark. Latency includes `Engine::handle` and its complete returned snapshot. It excludes fixture I/O, event cloning, expectation checks, output serialization/destruction, per-repetition initialization, and native UI. There is no warmup. Repetitions start with a fresh engine. Timing values can differ between runs. The recorded runs used a shared development host during other validation work, without CPU affinity or frequency control. RSS is not peak memory or allocation cost.

The older [39-entry fixture report](fixture-64.json) remains historical evidence, not a baseline for product performance. Current platform evidence and unresolved handoff are in the [validation register](../native-validation.md).
