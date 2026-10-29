# Fixtures

`lexicon.tsv` 是本專案自製的小型測試詞庫，依 repository 的 AGPL v3 license 散布。詞頻是測試數值，不是語料統計。

此詞庫只用於 replay、FFI、adapter contract 與開發 bundle。不得用它宣稱產品輸入品質或代表性效能已通過驗收。

JSONL case 每行包含 `event` 與選用的 `expect`。`expect` 是 Output 的遞迴 object subset；array 長度必須相同。省略 `expect` 不代表已驗證輸出。

回歸測試不得由目前程式輸出自動覆寫預期值。預期 preedit、commit 與 status 必須由 input spec 決定。

`cns11643.jsonl` 使用 `data/cns11643/lexicon.tsv`，不是此目錄的 39-entry fixture。其 81 個事件涵蓋新增字音、五種新增 rimes、一次 commit，以及 `ㄧˋ` 第 154 個候選的翻頁選取。預期值由 CNS 指定字區、固定排序規則與 input specification 決定，不由目前 engine 輸出回寫。

`ui-abort.jsonl` checks a balanced logical Escape tap followed by Shift mode changes. Windows adapter tests verify that candidate Abort sends the matching press and release.
