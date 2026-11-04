# Lexicon quality fixtures

本目錄只補齊缺少的 phrase heteronym 與 replay tie-order cases。`lexicon.tsv` 的 6 筆 entries、人工 scores 與 `cases.jsonl` 的 17 筆 expectations 均由本專案自行編寫，依既有 AGPL v3 policy 管理。未複製外部 dictionary、corpus 或 engine output。

本目錄使用獨立 lexicon。Root-level JSONL 與 native contract checks 仍使用 `../lexicon.tsv`，其 39 筆 baseline entries 與 SHA-256 不變。

## Coverage review

| 品質案例 | 已有證據 | 本次處理 |
| --- | --- | --- |
| Homophones | `../basic.jsonl` 的 `八`／`巴` 選字及 paging；core `explicit_selection_resolves_digit_tone_conflict_and_pages`。 | 沿用，不新增另一組 ranking case。 |
| Phrase heteronym | Core 只測試 single-character heteronym 可解析，沒有同一 phrase 的兩組 reading replay。 | 新增 `長大` 的 `ㄔㄤˊ ㄉㄚˋ` 與 `ㄓㄤˇ ㄉㄚˋ`。 |
| Length competition | Core `optimal_segmentation_is_not_greedy` 與 `unigram_normalization_can_prefer_frequent_singletons`；`../reselection.jsonl` 的 phrase／singleton span 改選。 | 沿用，不新增另一個 segmentation case。 |
| Tie ordering | Core `exact_reading_index_and_frequency_order` 與 `frequency_then_text_and_segmentation_ties_are_deterministic`；尚無固定同分 candidate array 的 replay。 | 新增 `常`／`長` 同分的完整 candidate array。 |
| Unknown fallback | `../basic.jsonl` 固定 canonical `ㄆㄚ` 與 `unknown_reading`；core `invalid_pending_unknown_reading_and_space_rules` 另固定 raw commit。 | 沿用，不重複新增。 |

## Hand-authored expectations

以下推導先於第一次 replay，未使用待測 engine 產生 gold。

- `t ; 6` 對應 `ㄔㄤˊ`。`常` 與 `長` 的 score 均為 100。
  - 同分時依 UTF-8 text 遞增排序：`常`（U+5E38）在 `長`（U+9577）之前。
  - TSV 故意先列 `長`，避免把檔案順序誤當 tie-break rule。
- `5 ; 3` 對應 `ㄓㄤˇ`，singleton 為 `長`。兩種 reading 都再輸入 `2 8 4`（`ㄉㄚˋ`）。
  - `ㄔㄤˊ ㄉㄚˋ` 表示長且大的事物；`ㄓㄤˇ ㄉㄚˋ` 表示成長。兩筆 phrase text 均為 `長大`，各自保留完整 reading。
  - 人工 score 總和為 2400。Phrase 機率 `1000/2401` 大於兩個 singleton 的乘積 `(100/2401)^2`，所以兩次 preedit 均為 `長大`。
- Composition 結尾的 candidate anchor 為第二音節，只列 `大`，span 為 `[1, 2)`。
  - `home` 後 anchor 為第一音節。Candidates 先列 score 1000 的 `長大`，再列 score 100 的 singleton；`ㄔㄤˊ` 的 singleton 仍為 `常`、`長`。
- 兩個 Han characters 各佔 3 UTF-8 bytes，故 phrase 結尾 cursor 為 6，`home` 後為 0。
- 每筆 event 明列 preedit、commit 與 status。只有兩次 `enter` 會 commit `長大`；其餘 commit 與全部 status 均為 null。

## Checks

```bash
cargo test -p ime-cli --test repository_fixtures --locked
cargo test -p ime-cli --test repository_fixtures --release --locked
cargo build -p ime-cli --locked
cargo build -p ime-cli --release --locked
mkdir -p target/lexicon-quality
./target/debug/ime-cli replay fixtures/lexicon-quality/lexicon.tsv fixtures/lexicon-quality/cases.jsonl > target/lexicon-quality/debug.jsonl
./target/release/ime-cli replay fixtures/lexicon-quality/lexicon.tsv fixtures/lexicon-quality/cases.jsonl > target/lexicon-quality/release.jsonl
cmp target/lexicon-quality/debug.jsonl target/lexicon-quality/release.jsonl
```

2026-11-05 本機執行上述 commands 均為 exit 0。Debug 與 release 的 `repository_fixtures` 各通過 3 tests；quality replay 各輸出 17 records，`cmp` 相同，stdout SHA-256 均為 `23971344c1e1df5747f98be790ecc6e81a603968cd105d02a5233d42a190c3b7`。Fixtures 的來源 hashes 見 [`data/SOURCES.md`](../../data/SOURCES.md#self-authored-fixtures)。

Replay stdout 只作驗證證據，不得複製回 expectations。這些檢查不證明正式詞庫品質、真實 phrase frequency、代表性效能或 native IME 支援。
