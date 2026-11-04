# Fixtures

`lexicon.tsv` 是本專案自製的小型測試詞庫，依 repository 的 AGPL v3 license 散布。詞頻是測試數值，不是語料統計。

此詞庫只用於 replay、FFI、adapter contract 與開發 bundle。不得用它宣稱產品輸入品質或代表性效能已通過驗收。

JSONL case 每行包含 `event` 與選用的 `expect`。`expect` 是 Output 的遞迴 object subset；array 長度必須相同。省略 `expect` 不代表已驗證輸出。

回歸測試不得由目前程式輸出自動覆寫預期值。預期 preedit、commit 與 status 必須由 input spec 決定。

`ui-abort.jsonl` checks a balanced logical Escape tap followed by Shift mode changes. Windows adapter tests verify that candidate Abort sends the matching press and release.

## Lexicon quality

[`lexicon-quality/`](lexicon-quality/README.md) 使用獨立的自製 lexicon 與人工 expectations。它只補齊 phrase heteronym 與 replay tie-order cases。既有 homophones、length competition 與 unknown fallback 沿用原 tests，不重複新增。Baseline `lexicon.tsv` 不變。

## Unicode ranges

`unicode/lexicon.tsv` 將單音節 `ㄧ` 對應到人工文字 `𠀀é😀`，score 1 也是測試值，不是真實 reading 或 frequency。`unicode/cases.jsonl` 由人工固定 Home／End、選字、單次 commit 與取消的結果。

此文字包含 non-BMP Han character、`e` 加 U+0301 combining mark，以及 emoji。UTF-8 長度為 `4 + 1 + 2 + 4 = 11` bytes；UTF-16 長度為 `2 + 1 + 1 + 2 = 6` code units。Core cursor 使用 UTF-8 byte offset；adapter 必須轉換為 native range 單位，不得假設一個音節只含一個 Unicode scalar。

這兩個子目錄必須各自搭配同目錄的 lexicon 與 JSONL。不得將它們加入使用 baseline lexicon 的 root-level replay glob。`repository_fixtures` test 分別檢查這兩組檔案。
