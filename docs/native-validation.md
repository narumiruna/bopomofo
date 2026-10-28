# Validation register

## Status

本專案仍是開發版本，**尚未完成 PLAN.md，也未通過任何 installed native IME 驗收**。

Rust tests、platform build、headless contract tests 與 desktop checks 是不同證據。不得用其中一項代替另一項，也不得將未測的 OS／架構／應用程式列為支援。

本輪本機驗證日期為 2026-10-28 UTC。實作起始 Git revision 為 `14f08344f510179d64060fef603610ab238bc719`，內容以下列 source snapshot 識別。已取得推送 source 與執行 native CI 的授權，但不包含 IME 安裝、註冊、啟用或 artifact 發布。Artifact hashes 識別實際建置，不是簽章信任或產品認證。

首次本機驗證與 CI commit `c6d2e2c` 的 source/build/fixture snapshot 包含 87 個路徑，SHA-256 為 `6de256861cf932740b1f0874e7b14133aae70737541c6823f05709f07145b2b8`。計算法為：以 `git ls-files --cached --others --exclude-standard -z` 列出 `.gitattributes`、Cargo manifests／lock、toolchain、`crates/`、`platforms/`、`fixtures/`、`.github/`，依路徑排序後，逐筆串接 `path + NUL + contents + NUL` 再取 SHA-256。

## Reproducible checks

在 repository root 執行。Windows clean checkout 先執行 `cargo build -p ime-windows --locked`；DLL loader test 需要同一 target／profile 的 DLL：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo test --workspace --all-targets --release --locked
cargo run -p ime-cli -- replay fixtures/lexicon.tsv fixtures/basic.jsonl
bash platforms/macos/scripts/test.sh
```

Windows source/API cross-check：

```bash
for target in x86_64-pc-windows-msvc i686-pc-windows-msvc aarch64-pc-windows-msvc; do
  cargo check -p ime-windows --target "$target" --all-targets --locked
  cargo clippy -p ime-windows --target "$target" --all-targets --locked -- -D warnings
done
```

這些 Windows 指令只做 type check，不會 link 或執行 Windows DLL。

Linux headless check：

```bash
docker build -t bopomofo-fcitx5-check -f platforms/linux/fcitx5/ci/Dockerfile platforms/linux/fcitx5/ci
docker run --rm --mount "type=bind,source=$PWD,target=/src,readonly" bopomofo-fcitx5-check
```

Container 只建立暫存 build directory，不安裝或啟用 IME，不連接 desktop session。Image 的 Ubuntu 24.04 base digest 已固定；實際 SDK／compiler package versions 仍須隨結果記錄。

## Verified evidence

| Check | 已取得的證據 | Scope / limit |
| --- | --- | --- |
| P1 early integration spike | 在 decoder 接入前完成 Swift bundle；以 ctypes 呼叫 C ABI，確認固定 `測試` preedit 與一次性 Enter commit。 | 固定測試路徑已被正式 engine 取代；未安裝或操作 native UI。 |
| Rust workspace | Debug／release 各 80 tests 通過：core 39、CLI 19、FFI 6、Windows portable 16；含 20,000-event bounded smoke、Unicode、locks、checksum 與單次 commit。 | 自製 fixture 與 deterministic generated cases，不代表正式詞庫品質。 |
| Rust format / Clippy | 本機 workspace format、Clippy warnings-as-errors 已通過。 | 最終 source 變更後仍須重跑；以 command exit status 為準。 |
| macOS arm64 bundle | CLT Swift 6.4 + Rust 1.92.0，plist／ABI／SHA-256／ad-hoc signature checks 通過。 | Deployment target 13.0 不是已驗證最低 OS。 |
| macOS standalone contracts | 125 pure adapter assertions、96 actual C ABI assertions、16 management dry-run cases 通過。 | 不建立 IMKServer session，不啟動輸入法，不執行 `--apply`。 |
| Windows x64 / x86 / ARM64 source | 三個 MSVC targets 均通過 pinned Rust 1.92.0 `cargo check` 與 target Clippy。 | 這是本機 cross-check；三架構 native build／load 的獨立 CI 證據見下節，installed TSF 仍未驗證。 |
| Windows portable contracts | 在 macOS 執行的 16 tests 通過，含 physical scan codes、post-event modifiers、offsets、checksum、probe、stale tickets 與 dispatch gate。 | 不是 COM／TSF native runtime 證據。 |
| Linux arm64 headless | Initial source snapshot clean build、ABI／C++ contracts 4/4 及 C++ ASan／UBSan 4/4 通過；含 Shift reset、native remap、Sensitive 與八份 fixture。 | ASan／UBSan adapter test 230.79 s；Rust archive 為正常 release，未使用 sanitizer instrumentation；不證明 desktop 或 index-only UI 安全。 |
| Native CI preparation | `.github/workflows/checks.yml` 與 `.github/workflows/windows.yml` 已啟用，3 份 YAML parse 通過。 | 已推送並觸發，首次 run 的失敗與重驗狀態見下節。 |
| PowerShell source | 官方 PowerShell 7.5.0 Linux amd64 container 解析 4 份 `.ps1` 成功。 | 此項僅為 AST parsing；`Build-Test.ps1` 的 Windows runtime 證據另列於 CI，註冊／移除腳本仍未執行。 |
| Documentation | 12 份 Markdown 的 whitespace／fences／local links 通過；8 份 Mermaid diagrams 已用 Mermaid CLI 11.12.0 + local headless Chrome render。 | Renderer outputs 在 ignored `target/diagram-check/`，不納入 Git。 |

## GitHub Actions

首次 [run 40651450673](https://github.com/narumiruna/bopomofo/actions/runs/40651450673) 對應 commit `c6d2e2cd330643832731221d8f45170cff8cb88e`：

| Job | Result | Scope |
| --- | --- | --- |
| `rust (ubuntu-24.04)` | Passed | Workspace format、Clippy 與 tests。 |
| `rust (macos-14)` | Passed | Workspace checks、bundle 與 standalone adapter／C ABI contracts。 |
| `fcitx5 (ubuntu-24.04)` | Passed | x64 build／ABI／4 headless CTests，沒有 desktop session。 |
| `fcitx5 (ubuntu-24.04-arm)` | Passed | arm64 build／ABI／4 headless CTests，沒有 desktop session。 |
| `rust (windows-2022)` | Failed | 19 Windows unit tests 通過；DLL loader test 回傳 `0x8007007E`。Workflow 未先 build 預期的 host DLL，後續 target-specific script 未執行。 |

修正加入 Windows host `cargo build -p ime-windows --locked` 前置步驟，並讓 loader test 先檢查 DLL 是否存在。

重驗 [run 40652597435](https://github.com/narumiruna/bopomofo/actions/runs/40652597435) 對應 commit `f65a705ea598fc20368851f8fe63096408df6a2d`，**5/5 jobs 全部通過**。此版本的 87-path source snapshot SHA-256 為 `50644afad0c8a97c14edd19c02e2c9cbbf236c8eea248b25a7581611ff4d180c`，計算法同上。

- Windows Server 2022 x64：workspace 84 tests 通過。Target-specific script 再次通過 19 unit tests 與 1 DLL loader test，完成 debug／release DLL build。Loader 驗證四個 exports、factory／service 建立及 unload count；沒有註冊或啟用 TSF。
- macOS 14.8.9 arm64，runner image `macos-14-arm64/20260831.0302.1`：workspace 80 tests、bundle、125 pure assertions、96 actual C ABI assertions 與 16 dry-run cases 通過。
- Ubuntu 24.04：workspace 80 tests 通過；x64 與 arm64 Fcitx5 jobs 各完成 build／ABI 與 4/4 headless CTests。
- 此 run 沒有 installed IME、真實 document edit session、UI-less app、native candidate rendering 或 desktop 相容性證據；這些 gate 保持 open。

Commit `084517df55c064015ff32249ceaa7c15eb0a382d` 的 [common run 40654797765](https://github.com/narumiruna/bopomofo/actions/runs/40654797765) **5/5 jobs 通過**，[Windows run 40654797923](https://github.com/narumiruna/bopomofo/actions/runs/40654797923) **3/3 jobs 通過**。此版本的 87-path source snapshot SHA-256 為 `03c9ebae5e5c141365c39921baca152238aec4917d177e03767eed9b811016be`。

| Windows target | Actual environment | Result / build job ID |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | Windows Server 2022，NT 10.0.20348.0，x64 | Debug／release build、19 unit tests、1 debug DLL loader test passed；job `122703118889`。 |
| `i686-pc-windows-msvc` | 相同 x64 OS，實際執行 WOW64 x86 test executables | 相同 19 + 1 tests passed；job `122703118421`。 |
| `aarch64-pc-windows-msvc` | Windows 11，NT 10.0.26200.0，Rust host `aarch64-pc-windows-msvc` | 相同 19 + 1 tests passed；job `122703118971`。 |

Logs 已確認三個 target directory 的 test executables 實際執行，不是只 cross-check。三列都未註冊、啟用或進行 desktop input。不能將 WOW64 結果推論到 x86 app integration，或將 native ARM64 結果推論到 ARM64 上的其他 emulated apps。

### Review and release-profile verification

Commit `be12f4804683a270932c484e5d0fce6dad304b26` fixes candidate Abort key balance, dynamic document restrictions, and context-pop focus restoration. [Common PR CI 40678543301](https://github.com/narumiruna/bopomofo/actions/runs/40678543301) passed 5/5 jobs. [Windows PR CI 40678543330](https://github.com/narumiruna/bopomofo/actions/runs/40678543330) passed 3/3 jobs. Each Windows target ran 25 unit tests and one debug DLL loader test. Its 89-path source snapshot SHA-256 is `d49cdd4337a3a6e19f0d3ec4a7b8cd90d7245fa1359f450f157f3a1e7c3b9308`.

Commit `64b6b5a8678660ac3b1ee2754b55bc17d08a68d0` adds release-profile tests and DLL hash logging. [Common PR CI 40681626779](https://github.com/narumiruna/bopomofo/actions/runs/40681626779) passed 5/5 jobs. [Windows PR CI 40681626909](https://github.com/narumiruna/bopomofo/actions/runs/40681626909) passed 3/3 jobs. Each Windows target ran 25 unit tests and one DLL loader test **in each debug and release profile**. Its 89-path source snapshot SHA-256 is `15576e969ebb7d4b9e087e55e9306350f236b92a0ec829a07861f5cbd423b9c0`.

The six new Windows tests use COM mocks. They cover dynamic flags, balanced Abort, held keys, error recovery, and context-stack focus. They do not test an installed TIP, a live text store, or candidate rendering.

The shared replay now has nine files and 358 records. Debug and release CLI replay checks passed. A Linux arm64 C++ ASan/UBSan follow-up passed the new 16-record `ui-abort.jsonl` case and the adapter contracts. It used `RelWithDebInfo`, `-fsanitize=address,undefined -fno-omit-frame-pointer`, leak detection, and halt-on-UB. The Rust archive was a normal release build. The contract executable SHA-256 was `851b4796dfc2ffc97062957bef1968ea4cb9730cb19ffe83e84df9c596520a60`. No desktop or stale rendered-click safety pass is claimed.

## Artifact identity

macOS executable：

```text
platforms/macos/build/Bopomofo Development.app/Contents/MacOS/BopomofoDevelopment
SHA-256 61cf0838d64a91a6359e8ae853a1bc8c98c447bba9bff801a34a18c4b2b532ca

Bundled lexicon.tsv
SHA-256 c54ced5e1e44472337f9bb5c5b8fe41e7a692a33dbfa8d360991704888f11dcd

Bundled lexicon.sha256
SHA-256 214a4ecdd73e1f785995dacaaad733b7796cd765819a159bb0524c0482e59f84

C ABI header
SHA-256 6a2480cbae7000618cba390a3932998595570a90f1a5729f4baab3bb93ff3167
```

每次重建後，以 `platforms/macos/build/build-evidence.txt` 的新 hash 核對本節。Build 輸出與 binary 不納入 Git。

Windows DLL hashes below identify the actual builds in run `40681626909` at commit `64b6b5a`. Both profiles passed loader tests. No Windows binary was downloaded, installed, or published.

| Windows target | Debug DLL SHA-256 | Release DLL SHA-256 | Build job ID |
| --- | --- | --- | --- |
| `x86_64-pc-windows-msvc` | `87398e91d21f8d4470f84b9b41e6f06a2f2586c93e17cf506cc0cc01d7dca450` | `f8e534cf2160506902063308ec3d721b328b2661e8181e2be4796931f42d1c5f` | `122793498086` |
| `i686-pc-windows-msvc` | `07e1787ac7678dccd21fa02a93afe5392fa4eb6a87df4d5536febdf3eb0bfd53` | `26ad12e96766596e2cd4eeb62efd29649a5904ce1f3555e121190187c1e5b811` | `122793497700` |
| `aarch64-pc-windows-msvc` | `766dd11a49f458977d0faf91f55ac25b5abb0e686eaacb4ae46083a6f3bb8fe8` | `3a35abd35aa730277a99a10a7e859f0dab3f500fe3476f449b58014511413e3d` | `122793498087` |

Each target used the self-authored lexicon SHA-256 `c54ced5e1e44472337f9bb5c5b8fe41e7a692a33dbfa8d360991704888f11dcd`. Hashes identify these builds, not a signature trust chain or product acceptance.

Linux 最終 clean container build：

```text
libbopomofo-dev.so
SHA-256 c1a9c527f9394a79120ded43c66d290c6008abe1a0e56752b494d8185d254eb7

release libime_ffi.a
SHA-256 0c7e94cf93b9674b1d3f6a6e55aa2456894d46c109d75692b47fe7e37f10c1ab

C++ ASan/UBSan libbopomofo-dev.so
SHA-256 5dad9d2c1838f05f1f35870feb1ad8d448304b6d1081fe46345c2023b2e36630

Ubuntu 24.04 base multiarch digest
sha256:24737cef17f4e22b1977df7d9a05e77c4e28b7da5b8911f8d8ccda136bf3573c

PowerShell 7.5.0 Linux image digest (AST parsing only)
sha256:042240d57ec9e47e511033b92625a8d95875ee5860af3015992c248b58a8be81
```

Linux 使用 GCC 13.3.0、CMake 3.28.3、Fcitx5 5.1.7、xkbcommon 1.6.0、nlohmann-json 3.11.3。獨立 release build 與 C++ sanitizer build 使用的 Rust archive hash 相同。兩者均使用相同 lexicon 與 header hashes。驗證用 container 已移除。

## Performance evidence

原始 report 保存在 [benchmarks/fixture-64.json](benchmarks/fixture-64.json)。可重跑：

```bash
cargo run --release -p ime-cli -- benchmark fixtures/lexicon.tsv fixtures/limit-64.jsonl 100
```

- Host：Apple M4、macOS 26.6.2、aarch64、Rust 1.92.0。
- Lexicon：39 entries、696 bytes，自製人工 frequency。
- Corpus：201 events，含最多 64 音節、超限、候選翻頁、刪除與 commit。
- 100 repetitions，共 20,100 timed key events。
- 初始化：87,208 ns，其中 lexicon parse 74,208 ns，Engine 建立 13,000 ns。
- Key handling p50／p95／p99：12,542／24,792／33,667 ns。
- Process RSS sample：3,506,176 bytes；不是 peak memory 或 allocation cost。
- 量測包含完整 `Engine::handle` 回傳 snapshot，不含 native UI、fixture I/O、expectation checks 或 event cloning。

**此 report 只證明 benchmark 工具與人工 stress case 可執行。正式詞庫與具代表性 corpus 的 p95 < 5 ms gate 仍為 unverified。** 不用這份小型 fixture 結果代替產品效能結論。

## Known Fcitx5 selection boundary

Core 與 captured CandidateWord callback 會驗證 generation。但 Fcitx5 5.1.7 D-Bus frontend 的 `SelectCandidate(index)` 只帶 index，並查詢當下的 candidate list。

另查核 [Fcitx5 5.1.14 pinned source](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/frontend/dbusfrontend/dbusfrontend.cpp)：`selectCandidate` 仍取 current list，D-Bus method signature 仍為 `i`，沒有 generation。該檔 SHA-256 為 `32755f97fa606353f11d49301e1d628f8d346c60e2ec3053b5d3c0505316b1c5`，因此不能假設只升級此版本就能解除限制。


A follow-up audit on 2026-10-28 UTC checked the newest official release found, **Fcitx5 5.1.23**. The [tag](https://github.com/fcitx/fcitx5/tree/27cca6e0239938b2d53937061d0980c087856b22) resolves to commit `27cca6e0239938b2d53937061d0980c087856b22`. The [official release archive](https://download.fcitx-im.org/fcitx5/fcitx5/fcitx5-5.1.23.tar.zst) SHA-256 is `d287119db9dc3113bb3a2b0e78ce8670e064b744c14d3aaec69799374aadb00b`.

- [SelectCandidate](https://github.com/fcitx/fcitx5/blob/27cca6e0239938b2d53937061d0980c087856b22/src/frontend/dbusfrontend/dbusfrontend.cpp#L403-L412) still resolves the index against the current list.
- Its input signature remains `i`. `UpdateClientSideUI` has no generation or render acknowledgement. The file SHA-256 is `48c1bb1aa3e6446804fe906312488c07e0f5a066be0910440af9884281b51194`; the archive copy matches it.
- The checked [GTK 5.1.7 client](https://github.com/fcitx/fcitx5-gtk/blob/3b18d2ab7401d4233daf38bbc5896f1703685a43/fcitx-gclient/fcitxgclient.c#L560-L571) and [Qt 5.1.16 client](https://github.com/fcitx/fcitx5-qt/blob/115750044f3d0db16ca5859808c7da830fd1ff52/qt5/dbusaddons/fcitxqtinputcontextproxyimpl.h#L109-L114) send asynchronous index-only requests.
- `KeyEventOrderFix` applies to key-event output. It does not bind a candidate click to a rendered list. FIFO delivery alone cannot prevent the server from replacing a list before the client displays it.

The audit found no sufficient generation or ordering guarantee. P6 remains blocked. No framework or client code was executed or imported.

```mermaid
sequenceDiagram
    participant UI as Native candidate UI
    participant Frontend as Fcitx D-Bus frontend
    participant Addon as Addon
    UI->>Frontend: SelectCandidate(index)
    Note over UI,Frontend: Request has no generation
    Frontend->>Addon: Select current CandidateWord at index
    Note over Frontend,Addon: Current list may differ from rendered list
```

因此，舊 CandidateWord 物件被拒絕的 headless test，不能證明延遲的舊 UI index request 也會被拒絕。這是已知的 protocol-level integration gap，不只是缺少 desktop 截圖。

在取得 framework／client ordering 或 generation 保證，或經授權完成相應設計變更前，不得宣稱已符合 stale rendered-click 要求。不得用新 token 掩蓋舊 UI request，也不自行加入被 scope 排除的 custom candidate window。

## Native matrix

以下各列均為 **unverified**。沒有執行安裝、註冊、啟用、移除或系統設定變更。

| OS / architecture | Application / frontend | 必須驗證的行為 |
| --- | --- | --- |
| macOS arm64 | Native text app | marked text、UTF-16 cursor、候選、一次 commit、取消、focus、secure fields。 |
| macOS arm64 | Browser | 同上，另測表單、快捷鍵、input-source 切換。 |
| macOS arm64 | Electron | 同上，記錄 app／Electron version。 |
| macOS arm64 | Scaling / multi-monitor | 候選定位、縮放、螢幕邊界與切換。 |
| macOS x86_64 | 各類 app | Build 與 native 行為均獨立驗證；不沿用 arm64 結果。 |
| Windows x64 | Native text / browser / Electron | DLL load、COM lifetime、probe、edit sessions、UI-less 與 native listbox、focus、restrictions。 |
| Windows x86 | WOW64 / x86 app | 相應 DLL build／load 與上述操作；不能沿用 x64 app 結果。 |
| Windows ARM64 | Native / emulated app | 相應 build、process architecture、candidate 與 lifecycle。 |
| Linux arm64 | X11 GTK / Qt / browser / Electron | Addon load、preedit、candidate、commit、focus、Sensitive／Password 與移除。 |
| Linux arm64 | Wayland GTK / Qt / browser / Electron | 同上，另記 compositor、protocol、frontend 與 toolkit module。 |
| Linux x86_64 | X11 / Wayland 各組合 | Headless build、desktop 與 app 各自驗證；不沿用 arm64 結果。 |
| All | Delayed native callback / selection | Reset、換 context／candidate list 後不可提交或修改新 composition；Fcitx index-only gap 保持 open。 |

每筆結果須包含 OS／app／toolkit version、CPU architecture、source/build ID、artifact／lexicon SHA-256、固定 fixture、預期、實際結果與 passed／failed／unverified。只使用自製測試文字，不收集使用者日常輸入。

## External handoff and release restrictions

| Responsible role | Prerequisite / required action | 尚未通過的 gate |
| --- | --- | --- |
| Windows tester / CI owner | 三架構 build／load／非註冊 tests 已通過；提供已授權的 installed TSF／app 環境。 | Registration、activation、實際 document edit session、candidate rendering 與各架構 desktop matrix 仍未驗證。 |
| macOS tester | 授權使用 scoped 管理腳本安裝／啟用 bundle，依 macOS guide 操作與保存結果。 | 所有 installed native app／display checks 未驗證。 |
| Linux tester / framework owner | 提供 X11／Wayland session，驗證各 frontend，並解決或證明 candidate request 的 generation／ordering 契約。 | Desktop matrix 與 stale rendered-click 正確性未通過。 |
| Data / release owner | 提供權利完整、可轉換與散布的 phrase／frequency data、notices 與代表性 corpus。 | 正式詞庫、代表性效能及資料散布 gate 未通過；詳見 `data/SOURCES.md`。 |
| Release owner | 在各 gate 通過後另行授權 signing／notarization／發布。 | 本輪不發布，也不提供產品支援聲明。 |

這些限制未解除前，保留 PLAN.md 的未完成項目，不以文件完成代替實作與驗收。
