# bopomofo

跨平台 Traditional Chinese Zhuyin IME。共用輸入引擎以 Rust 自行實作，不使用現成輸入引擎。

目前是開發版本，**不可宣稱已通過日常使用或 native IME 驗收**。正式詞庫、代表性效能與各平台 desktop checks 仍須依驗證紀錄確認。

## Components

- `ime-core`：標準 Dachen、組字狀態、詞庫索引、dynamic-programming decoder 與候選排序。
- `ime-ffi`：macOS／Linux 共用的 C ABI。
- `ime-cli`：非互動 replay、詞庫驗證、正規化建置與 benchmark。
- Native adapters：macOS InputMethodKit、Windows TSF、Linux Fcitx5。

每個 input context 使用獨立 Engine。輸入處理完全離線，不記錄或傳送按鍵、preedit 或 committed text。

## Core development

需要 Rust 1.92.0，版本由 `rust-toolchain.toml` 固定。Windows 執行下列 workspace tests 前，先執行 `cargo build -p ime-windows --locked`，建立 DLL loader test 所需的 host DLL。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p ime-cli -- replay fixtures/lexicon.tsv fixtures/basic.jsonl
```

Replay 每行輸出一份 JSON Output。它只處理明確提供的測試事件，不連接系統鍵盤。

## Lexicon tools

```bash
cargo run -p ime-cli -- validate-lexicon fixtures/lexicon.tsv
cargo run -p ime-cli -- build-lexicon fixtures/lexicon.tsv target/lexicon.tsv
cargo run --release -p ime-cli -- benchmark fixtures/lexicon.tsv fixtures/limit-64.jsonl 100
```

詞庫格式為 UTF-8 TSV。第一行是 `# bopomofo-lexicon-v1`。每筆資料包含 reading、text 與正整數 frequency，以 Tab 分隔。Reading 內的 canonical syllables 以單一空白分隔。第一聲省略符號，其餘使用 `ˊ`、`ˇ`、`ˋ`、`˙`。

`build-lexicon` 驗證資料後，以固定順序匯出並回報 SHA-256。失敗時不覆寫既有產物。Native package 另附 checksum manifest，於初始化時檢查資料完整性。

`fixtures/lexicon.tsv` 保留為自製測試資料。macOS 預設開發 bundle 改用 CNS11643 Taiwan 常用／次常用字補充：13,052 字、16,496 字音 pairs，加上原有五筆自製詞組。資料來源、獨立授權與離線重建方式見 [CNS11643 guide](data/cns11643/README.md)。

CNS 字音與 fixture scores 都不是實測 phrase frequency。此資料可補充單字候選，但不能證明正式詞組品質、代表性效能或 native IME 驗收。

## Platform builds and installation

Build、安裝、啟用與 native 驗證是不同步驟。建立 bundle／DLL／addon 不會自動啟用輸入法。

- [macOS](docs/macos.md)：Swift host、bundle、adapter tests、安裝與還原步驟。
- [Windows](docs/windows.md)：TSF DLL、COM registration、架構與應用程式限制。
- [Linux](docs/linux.md)：Fcitx5 addon、CMake、headless tests 與 desktop matrix。

實際安裝、註冊、啟用、移除、系統設定變更與發布均須取得明確授權。管理腳本預設只執行 dry-run。

## Documents

- [Architecture](docs/architecture.md)
- [Input specification](docs/input-spec.md)
- [Data sources and licenses](data/SOURCES.md)
- [Platform prerequisites](docs/platform-support.md)
- [Native validation](docs/native-validation.md)
- [Release workflow](docs/releases.md)
- [Development rules](AGENTS.md)

## License

專案保留現有 GNU AGPL v3 license。外部詞庫資料須分別保留適用的 license、copyright 與 notices，不因放入此 repository 而改成 AGPL。
