# Fixtures

`lexicon.tsv` 是本專案自製的小型測試詞庫，依 repository 的 AGPL v3 license 散布。詞頻是測試數值，不是語料統計。

此詞庫只用於 replay、FFI、adapter contract 與開發 bundle。不得用它宣稱產品輸入品質或代表性效能已通過驗收。

JSONL case 每行包含 `event` 與選用的 `expect`。`expect` 是 Output 的遞迴 object subset；array 長度必須相同。省略 `expect` 不代表已驗證輸出。

回歸測試不得由目前程式輸出自動覆寫預期值。預期 preedit、commit 與 status 必須由 input spec 決定。
