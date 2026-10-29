# CNS11643 Taiwan character supplement

The macOS development bundle uses this single-character supplement. It is not a production phrase lexicon. McBopomofo remains investigation-only; no McBopomofo data or engine code is imported.

## Approved source and license

The user approved CNS11643 use on 2026-10-29. The [official dataset](https://data.gov.tw/dataset/5961) explicitly covers Zhuyin attributes and encoding mappings. Its remarks permit a choice of OGDL-Taiwan-1.0 or OFL-1.1. This import selects **Open Government Data License, version 1.0**.

Sections 2.1–2.3 permit compilation, adaptation, distribution, and sublicensing without a separate written grant. Keep [NOTICE.txt](NOTICE.txt) and the complete [LICENSE.txt](LICENSE.txt). The [CNS license page](https://www.cns11643.gov.tw/pageView.jsp?ID=59) also requires source attribution and agreement to provide discovered errors or omissions for correction without charge. Do not imply official endorsement.

The project and self-authored seed phrases keep their existing AGPL-3.0-only license. No external release is authorized.

## Character scope

The [official encoding description](https://www.cns11643.gov.tw/pageView.jsp?ID=9) defines these character regions:

| CNS region | Selected codes | Basis |
| --- | --- | --- |
| Plane 1 | `4421–7D4B` | 5,401 common characters from MOE standard forms, textbooks, and six variants. |
| Plane 2 | `2121–7245` | Less-common characters based on MOE character tables. |

Only these regions are retained. Plane 1 symbols and letters, later planes, user-defined characters, and Big5 extensions are not imported. Import validates every retained Unicode mapping as a distinct BMP Han character.

This is a Taiwan standard common/less-common subset, **not a claim that every shared or historical form is forbidden in Simplified Chinese**. It includes legitimate shared/historical forms such as `台`, `后`, `干`, `里`, and `体`. It excludes sampled later simplified forms such as `汉`, `国`, `学`, `湾`, `龙`, `语`, and `发`. Rare and newer Taiwan characters outside the selected regions remain unavailable.

The ranges include `2-7245` → `礴` even though the current core Big5 mapping omits that code. Big5 membership is not required.

## Pinned source snapshot

Retrieved on **2026-10-29 UTC**. The official [release record](https://data.cns11643.gov.tw/opendata/release.txt) identifies both archives as version **20261028**. Checksums below were calculated locally; they are not publisher signatures. Download URLs can change.

| File | SHA-256 |
| --- | --- |
| [Properties.zip](https://data.cns11643.gov.tw/opendata/Properties.zip) | `91214ba96946e0ea41d4cb7b709c036f3301dd4a621d9a0fa6ad2ccf1db6d297` |
| [MapingTables.zip](https://data.cns11643.gov.tw/opendata/MapingTables.zip) | `605cadc2074cb5399bf03a268ab1f66a52384f08acb27021178c13d041cb87d2` |
| Original `CNS_phonetic.txt` | `98820d0df099329fc4da2c4756074530d989cae783dcbdd0652097083283f352` |
| Original `Unicode/CNS2UNICODE_Unicode BMP.txt` | `9f0d89dc002f0848e4ab88704b4940c5b74d9321eb4ccf1a1283d440cf73b782` |

The two retained source tables preserve the original row bytes and order for the selected CNS regions. They do not copy other repertoire sections.

| Retained file | Rows | Bytes | SHA-256 |
| --- | --- | --- | --- |
| [source/phonetic.tsv](source/phonetic.tsv) | 16,497 | 268,665 | `2d05d08fb0045d61c83253c66a913b9f6ce349236803c4b662e715ff4cadbb1a` |
| [source/unicode.tsv](source/unicode.tsv) | 13,052 | 156,624 | `eb14c6f80a83be72300e4c4c5bbf8f02159d5822bd15001850f523c297e90d3c` |

The importer verifies both retained files against [source/manifest.json](source/manifest.json) before conversion. This detects mismatched bytes, not a malicious replacement of both files and manifest. Repository review and the upstream provenance record remain necessary.

### Recover the retained tables from the official archives

Use a temporary directory. Download the two archives above and verify their SHA-256 values before extraction. Do not replace a pinned snapshot with a changed download.

The source members are `CNS_phonetic.txt` in Properties.zip and `Unicode/CNS2UNICODE_Unicode BMP.txt` in MapingTables.zip. For each source file, keep a row only when its first Tab-separated field satisfies:

```text
CNS plane = 1 and 0x4421 <= code <= 0x7D4B
or
CNS plane = 2 and 0x2121 <= code <= 0x7245
```

CNS row and column bytes must both be in `0x21..=0x7E`. Keep each selected row's original bytes, including its final LF. The retained file hashes must match the table above. Binary archives are not stored in Git.

## Import rules and output

- First tone has no mark. Leading neutral-tone `˙` becomes the canonical trailing mark.
- Preserve separate readings. Reject duplicate mappings/readings, missing mappings/readings, malformed records, unsupported syllables, non-Han mappings, and out-of-scope codes.
- The published `2-3378 / 啐 / ˙ㄑ` row has no complete syllable. Retain it in the source snapshot but report one exclusion. Do not guess `ㄑㄧ˙`. Other valid readings preserve the character.
- The core parser accepts the additional source-backed rimes `ㄆㄧㄚ`, `ㄈㄧㄠ`, `ㄋㄨㄟ`, `ㄌㄩㄢ`, and `ㄙㄟ`.
- Character score is `30000 - (plane * 8836 + (row - 0x21) * 94 + column - 0x21)`.
  - Common-plane characters rank before less-common characters, then CNS stroke/radical order applies.
  - These scores are not corpus frequencies or a Taiwan usage-quality claim.
- Keep only the five multi-syllable phrases from the self-authored fixture. Every phrase character and reading must match the selected CNS data.
  - Ignore the fixture's 34 single-syllable entries. Test-only readings and scores must not replace source data.
  - Phrase scores remain artificial.
- The core snapshot bound is 256 candidates, with nine per page. The source has 154 candidates for `ㄧˋ`; the old 128 bound would hide valid entries.
- Write the lexicon atomically only after all checks pass. Refuse output paths that name an input file.

[lexicon.tsv](lexicon.tsv) has **13,052 characters**, **16,496 character-reading pairs**, and **five self-authored phrases**: 16,501 entries, 311,786 bytes.

SHA-256: `f279ff2faa8bc40814e300253cda97c162003a0b6650fdfa0484f4075d0b300a`.

## Offline rebuild and checks

Run from the repository root:

```bash
cargo run --locked -p ime-cli -- import-cns11643 \
  data/cns11643/source fixtures/lexicon.tsv target/cns11643.tsv
cmp data/cns11643/lexicon.tsv target/cns11643.tsv
cargo run --locked -p ime-cli -- validate-lexicon target/cns11643.tsv
cargo run --locked -p ime-cli -- replay data/cns11643/lexicon.tsv fixtures/cns11643.jsonl
cargo test --locked -p ime-cli --test cns11643
bash platforms/macos/scripts/test.sh
```

[build.json](build.json) records the repeatable import summary, including source/seed hashes and the explicit exclusion count. Tests rebuild twice and compare bytes and reports. The 81-event replay covers `我`, `國`, the new rimes, the retained `你好` phrase, and selection of `齸` at candidate index 153.

No download occurs during these checks or during key processing. The macOS bundle includes its data notice and license. Fixture-only adapter tests use the original fixture separately.

## Native handoff and remaining gates

Rebuilding does not update an installed input method. The user must separately authorize any installed-bundle update, registration, activation, removal, or settings change.

Actual candidate rendering, mouse callbacks, application compatibility, focus behavior, and installed use remain unverified. Follow [macOS validation](../../docs/macos.md). Native checks must cover more than 128 candidates and the new character readings with this lexicon hash.

A data maintainer can report the incomplete upstream reading through the official CNS contact after obtaining authorization for that external action. This task records the issue but does not submit it.

A complete Taiwan phrase lexicon, measured frequencies, representative performance, and native acceptance remain open. See [source records](../SOURCES.md) and [native validation](../../docs/native-validation.md).
