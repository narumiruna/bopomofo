# bopomofo

跨平台 Traditional Chinese Zhuyin IME。Rust `ime-core` 封裝固定版本的 **libchewing 0.13.1**。macOS InputMethodKit、Windows TSF 與 Linux Fcitx5 adapters 共用輸入契約。

目前是開發版本。**尚未通過 installed native IME 驗收**。正式詞庫品質、代表性效能與應用程式相容性仍須獨立驗證。

## Components

- `ime-core`：libchewing Editor、context-local state、mode／modifier、安全限制、read-only probe 與 UI selection 契約。
- `ime-ffi`：macOS／Linux 使用的 C ABI v2；從已驗證的 memory assets 建立 engine。
- `ime-cli`：離線 assets builder、資料驗證、非互動 replay 與 benchmark。
- `ime-test-support`：僅供 tests 使用的 fixtures compiler。
- Native adapters：只處理 OS 整合、資源載入、offset conversion 與 native presentation。

沒有自研 parser／decoder runtime 或可選 backend。每個 input context 有獨立 Editor。輸入處理離線，不記錄或傳送 keystrokes、preedit 或 committed text。User dictionary persistence、learning 與上游 logging 均停用。

## Development checks

需要 Rust 1.92.0，版本由 `rust-toolchain.toml` 固定。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo test --workspace --all-targets --release --locked
uv run --offline --no-project python -m unittest discover -s scripts/tests
```

Windows 先在各 profile 執行 `cargo build -p ime-windows --locked` 與 `cargo build -p ime-windows --release --locked`，供同 profile 的 DLL loader tests 使用。Cross-target `cargo check` 不等於 Windows native build。

## Runtime assets and replay

三平台預設使用所選的上游 `word.csv`／`tsi.csv`，不是 mini dictionary 或原 CNS supplement。資料版本、逐檔來源、LGPL grant 與 exclusions 見 [data sources](data/SOURCES.md#libchewing-default-data) 與 [notice](data/libchewing/NOTICE.md)。本輪有 160,440 個 merged entries；沒有保證 Taiwan corpus 品質，也沒有繁簡轉換功能。

```bash
cargo run -p ime-cli --locked -- build-assets data/libchewing/source target/default-assets
cargo run -p ime-cli --locked -- build-assets fixtures/lexicon.tsv target/fixture-assets
cargo run -p ime-cli --locked -- replay target/fixture-assets fixtures/basic.jsonl
cargo run --release -p ime-cli --locked -- benchmark target/fixture-assets fixtures/limit-64.jsonl 100
```

Assets 包含 `dictionary.dat`、`assets.json` 與分開的 project／engine／data licenses、authors／notices。Manifest 固定 engine、builder、source identity、filename、size 與 SHA-256。初始化拒絕缺檔、混版、hash mismatch 與不合法的 dictionary。Checksum 不提供來源認證。

Replay 只讀明確提供的 synthetic events，不連接系統鍵盤。Baseline、CNS11643、quality 與 Unicode 各用自己的 assets。配對與手寫 expectations 見 [fixture notes](fixtures/README.md#replay-pairing)。

`validate-lexicon`、`build-lexicon` 與 `import-cns11643` 仍是離線 authoring tools；TSV 不再是 runtime 輸入格式。未識別的 caller-supplied data 不會自動取得 project AGPL license 或 redistribution grant。匯入與散布前必須確認其來源與授權。

## Platform checks

Build、installation、activation 與 desktop validation 是不同步驟。Build 不會啟用 IME。

- [macOS](docs/macos.md)：Swift bundle、C ABI contracts、管理 dry-run 與 native handoff。
- [Windows](docs/windows.md)：TSF、各 MSVC target、DLL loader 與 registration 限制。
- [Linux](docs/linux.md)：Fcitx5、headless CTest、privacy trace、sanitizers 與未解的 DBus index-only selection gap。

實際安裝、註冊、啟用、移除、系統設定變更與發布均須另外取得明確授權。管理腳本預設 dry-run；不使用測試結果推定 OS／CPU／app 支援。

## Documents

- [Architecture](docs/architecture.md)
- [Input specification and migration differences](docs/input-spec.md)
- [Data sources and licenses](data/SOURCES.md)
- [Platform prerequisites](docs/platform-support.md)
- [Validation register and external handoff](docs/native-validation.md)
- [Fixture-only benchmark reports](docs/benchmarks/README.md)
- [Source-only release workflow](docs/releases.md)
- [Development rules](AGENTS.md)

## License

Project code 保留 AGPL-3.0-only。libchewing engine 與所選上游資料保留 LGPL-2.1-or-later；CNS 與自製 fixtures 使用各自的 license。不得因資料進入 repository 而更換其授權。任何未來 binary distribution 仍須完成 notices、corresponding source 與 static-link relinking obligations 的 release review。
