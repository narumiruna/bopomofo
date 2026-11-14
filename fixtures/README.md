# Fixtures

`lexicon.tsv` 是自製的 39-entry baseline，使用 repository 的 AGPL-3.0-only。Scores 是測試值，不是 corpus 統計。三平台 default build 已改用所選上游資料；baseline native smoke 必須明確建立 fixture package。

每行 JSONL 包含 `event` 與選用的 `expect`。Expected object 是遞迴 subset；array 長度必須相同。省略 `expect` 不等於通過驗證。Expectations 依 [input specification](../docs/input-spec.md) 手寫，不得從 actual output 自動覆寫。

## Replay pairing

每組 source 先以 `ime-cli build-assets` 編譯到獨立 directory，再傳給 replay。TSV 不再直接提供給 runtime。

| Cases | Authoring source |
| --- | --- |
| `basic`、`tones`、`modes`、`editing`、`limit-64`、`gestures`、`reselection`、`aliases`、`ui-abort` `.jsonl` | `lexicon.tsv`，39 entries。 |
| `cns11643.jsonl` | `../data/cns11643/lexicon.tsv`。 |
| `lexicon-quality/cases.jsonl` | `lexicon-quality/lexicon.tsv`。 |
| `unicode/cases.jsonl` | `unicode/lexicon.tsv`。 |

Root-level glob 排除 CNS case，但 CNS、quality 與 Unicode 都必須另行執行。Wrong-profile negative test 必須拒絕 CNS case 配 baseline assets，不得以移除 coverage 解決失敗。

```bash
cargo run -p ime-cli --locked -- build-assets fixtures/lexicon.tsv target/fixture-assets
cargo run -p ime-cli --locked -- replay target/fixture-assets fixtures/basic.jsonl
```

CNS 的 81 個 events 涵蓋 `我`、`國`、`你好`、新增 rimes 與 `ㄧˋ` 第 154 個候選。17 次 Page Down 後按 `1` 選取 `齸`，Enter 只 commit 一次。原 CNS source bytes 與授權不變。

## Selection and quality

上游一次提供一個 selection point。`editing`／`reselection` 明確用 Tab 改為單音節候選，不假設不同 span 混列。`ui-abort` 保留 balanced Escape press／release 與後續 Shift checks。

[Quality cases](lexicon-quality/README.md) 保留自製 heteronym 與 tie-order expectations。Default data、fixture ranking 或 synthetic stress 都不證明 Taiwan corpus 品質。Unknown reading 現在保留可編輯 spelling，不自動產生可提交的 raw composition。

## Unicode ranges

Unicode source 有兩筆自製資料：單音節 `ㄧ` 對應 `𠀀`；四個 `ㄧ` 對應四個 scalars `𠀀é😀`。Scores 不是實際 frequency。

19-record replay 手寫四次 `u Space`、Home／End、Home／Down／1／End、單次 commit 與取消。選字 span 為 `[0,4)`。上游選字保留原 cursor；因此選字後明確用 End 移到結尾。

完整文字包含 non-BMP Han、`e`、U+0301 combining mark 與 emoji。UTF-8 長度為 `4 + 1 + 2 + 4 = 11` bytes；UTF-16 長度為 `2 + 1 + 1 + 2 = 6` units。Core 測試也檢查 phrase 內部、combining scalar 前的 byte offset 5。不得混用 bytes、scalars、graphemes 與 UTF-16 units。

原本單音節對應四 scalars 的 source 不符合上游模型，asset build 必須拒絕。新的相容表示保留完整 Unicode replay；adapter synthetic range tests 仍檢查任意文字與非法 offsets，不刪除 emoji／combining coverage。
