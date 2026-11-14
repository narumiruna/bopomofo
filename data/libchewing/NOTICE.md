# libchewing notices

The input engine is the unmodified Rust crate `chewing` 0.13.1, licensed under LGPL-2.1-or-later. Upstream source: https://codeberg.org/chewing/libchewing, tag v0.13.1, commit f071d2e95c531cc39bb4325cd5a5d0e1d51daef8. The crates.io archive SHA-256 is `0fc533f0b524ce84ec20fe9025169e64c866df84a50b497acb6a62f6dccac264`.

The engine's embedded `src/editor/data/mini.dat` declares `Copyright (c) 2022 libchewing Core Team`, LGPL-2.1-or-later, data version `0.9.1-30-ga2d7fa9`, builder `chewing-cli 0.9.1`. Its SHA-256 is `1972c2ae6ca7e933b27c63c47220ae2a9889d353fd59ef7d22b26dc17d0643fc`. bopomofo does not use its fallback constructor.

The retained `source/word.csv` and `source/tsi.csv` come from https://codeberg.org/chewing/libchewing-data at commit `bfba418ea8a4bd2fe10b9bf80b4bf81ec01af5f4`, the data revision selected by engine v0.13.1. Each source file explicitly declares `Copyright (c) 2025 libchewing Core Team` and LGPL-2.1-or-later. The original metadata headers are retained without modification. No MOE dictionary, archive, or other upstream dictionary is included.

- `word.csv`: version 2026.3.20, 399905 bytes, SHA-256 `da55b8e599c1389bc486453554f3410cf9c621d0ffff0ce38855698d26b3892a`.
- `tsi.csv`: version 2025.11.17, 5244487 bytes, SHA-256 `c889a1ac3ae1901b3f8f62748bc41b958f010bf995f7f88dbaf9e3494f341428`.

LGPL sections 1–2 permit copying, conversion and redistribution with the required notices and corresponding source. Derived dictionaries retain LGPL-2.1-or-later. The build trims reading-field whitespace, normalizes syllables through the upstream parser, merges identical reading/text pairs using their maximum supplied score, and compiles the result with the pinned upstream TrieBuilder. It does not infer readings or claim that scores are measured modern Taiwan phrase frequencies. Upstream variants, including simplified and historical forms, remain present.

The project and its self-authored fixtures remain AGPL-3.0-only. Engine and data licenses are independent. Preserve this notice, `COPYING`, `AUTHORS`, source snapshots and reproducible build instructions with distributions. Static native binary distribution additionally requires the applicable LGPL relinking/corresponding-source materials; a license notice or Cargo.lock alone is not sufficient. No binary publication is authorized by this change.
