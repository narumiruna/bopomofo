# Platform prerequisites and support status

## Scope and status

**尚未宣稱任何 OS、CPU architecture 或 application combination 已通過 native IME validation。** 本文件記錄 SDK 調查與 build 前提，不是支援清單。Core、platform build 與 native desktop checks 必須分開提供證據，責任邊界依 [architecture.md](architecture.md)。

調查與本機檢查日期：**2026-10-28 UTC**。已查閱官方 API 與 local SDK，並建立 macOS bundle、Rust checks 與 Linux headless environment。未安裝、註冊、啟用或移除 native IME，也未切換系統 input settings。

最新 command、artifact 與限制以 [native-validation.md](native-validation.md) 為準。以下 SDK 調查細節不取代 native 驗證。

| Target | 已確認的 build 前提 | Platform build / native 狀態與缺少條件 |
| --- | --- | --- |
| Core / C ABI，`aarch64-apple-darwin` | **Available**：Rust/Cargo 1.92.0 | Core／FFI tests 已執行；SDK probe 仍不能代替這些 tests。 |
| macOS arm64 / InputMethodKit | **Available**：Swift、Clang、AppKit、InputMethodKit headers/link stubs | SDK typecheck、bundle build 與 standalone adapter tests 通過；native checks 仍需安裝／啟用授權及 desktop 操作。 |
| macOS x86_64 / InputMethodKit | **Blocked**：未安裝 `x86_64-apple-darwin` Rust target，未確認相應 runner | 尚無 x86_64 build 或 native 結果；arm64 結果不能推論到 Intel Mac。 |
| Windows x64，`x86_64-pc-windows-msvc` | Cross-check 與 Windows Server 2022 native CI **Available** | Debug/release DLLs and 25 unit tests plus one loader test per profile passed. Installed TSF and desktop input remain unverified. |
| Windows x86，`i686-pc-windows-msvc` | Cross-check 與 Windows Server 2022 WOW64 CI **Available** | x86 debug/release DLLs and 25 unit tests plus one loader test per profile passed under WOW64. This is not installed x86 app acceptance. |
| Windows ARM64，`aarch64-pc-windows-msvc` | Cross-check 與 Windows 11 ARM64 CI **Available** | ARM64 debug/release DLLs and 25 unit tests plus one loader test per profile passed. Native and emulated app combinations remain unverified. |
| Linux arm64 / Fcitx5 | Docker headless environment **Available**；macOS host 不直接安裝 Fcitx5 SDK | 已建立 Ubuntu 24.04／Fcitx5 5.1.7 headless environment；desktop session 與已知 integration gaps 仍未通過。 |
| Linux x86_64 / Fcitx5 | Ubuntu 24.04 native headless CI **Available** | Build／ABI／4 CTests 通過；沒有 desktop session，不推論到 GTK／Qt／browser／Electron。 |

`Available` 只表示指定的 development prerequisites 可使用，不表示產品支援。後續實作的測試結果須另附 command、revision、exit status 與 artifact 識別；不能由本表預先標為 passed。

## Actual host tooling

以下是實際 command output，不是最低支援版本：

| 項目 | 查核結果 |
| --- | --- |
| Host | macOS **26.6.2**，build **25G83**，`uname -m` 為 `arm64`。 |
| Rust | `rustc 1.92.0 (ded5c06cf 2025-12-08)`；`cargo 1.92.0 (344c4567c 2025-10-21)`。 |
| Installed Rust targets | `aarch64-apple-darwin`，以及 x64／x86／ARM64 三個 `pc-windows-msvc` targets；Windows targets 僅用於 source/API check。 |
| Default Swift | Apple Swift **6.4**，`swiftlang-6.4.0.34.1`，target `arm64-apple-macosx26.0`。 |
| Default Clang | Apple Clang **21.0.0**，`clang-2100.3.34.2`。 |
| Active developer directory | `xcode-select -p`：`/Library/Developer/CommandLineTools`。 |
| Default macOS SDK | `xcrun --show-sdk-path`：`/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk`；SDK version **27.0**。 |
| Full Xcode | `/Applications/Xcode.app` 存在；以 process-only `DEVELOPER_DIR` 查得 Xcode **26.6**，build **17F113**，SDK **26.5**。 |
| `xcodebuild -version` | 在目前 CLT selection 下失敗，訊息為 requires Xcode。不是 InputMethodKit SDK 缺失，也不是 full Xcode 不存在。 |
| Linux build tools on host | `cmake`、`ninja`、`fcitx5` 未在 PATH；`pkg-config --version` 為 **3.0.7**，`pkg-config --exists Fcitx5Core` 回傳 **1**。 |
| Windows build tools on host | 未找到 `cl.exe`、`link.exe` 或 `lld-link`。macOS `/bin/link` 不是 MSVC linker。 |
| Docker | Client/server **29.4.0**；context 為 `orbstack`，server 為 Linux `arm64`。已建立與執行 isolated headless container；不據此宣稱有 Fcitx5 desktop runner。 |

可重跑的唯讀查核：

```bash
sw_vers
uname -m
rustc --version
cargo --version
rustup target list --installed
swift --version
clang --version
xcode-select -p
xcrun --show-sdk-path
xcrun --show-sdk-version
# 只對此 process 指定 Xcode，不執行 xcode-select --switch。
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild -version
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun --show-sdk-version
pkg-config --exists Fcitx5Core
docker version
```

不同 SDK selection 會改變 build inputs。每次 build 都須記錄實際 `DEVELOPER_DIR`、SDK path/version、Rust target 與 compiler version；不能將兩套 SDK 的結果混為一組。

## macOS: InputMethodKit

### Verified API references

已以 `curl` 取得 Apple 官方 [InputMethodKit](https://developer.apple.com/documentation/inputmethodkit)、[IMKServer](https://developer.apple.com/documentation/inputmethodkit/imkserver)、[IMKInputController](https://developer.apple.com/documentation/inputmethodkit/imkinputcontroller) 與 [IMKCandidates](https://developer.apple.com/documentation/inputmethodkit/imkcandidates) 文件。另取得 Apple 的 [InputMethodKit documentation JSON](https://developer.apple.com/tutorials/data/documentation/inputmethodkit.json) 與各 class 的 JSON，並核對本機 SDK declarations。

本機查閱的 header paths，以 `SDK=$(xcrun --show-sdk-path)` 為基準：

```text
$SDK/System/Library/Frameworks/InputMethodKit.framework/Headers/IMKServer.h
$SDK/System/Library/Frameworks/InputMethodKit.framework/Headers/IMKInputController.h
$SDK/System/Library/Frameworks/InputMethodKit.framework/Headers/IMKCandidates.h
$SDK/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Headers/IMKInputSession.h
```

| 功能 | 具體 SDK API 與 adapter 要求 |
| --- | --- |
| Server / session | `IMKServer.init(name:bundleIdentifier:)`；server 依 bundle 資訊為 input sessions 建立 `IMKInputController`。每個 controller 持有獨立 core context。 |
| Key delivery | SDK 提供 `inputText:client:` + `didCommandBySelector:client:`、`inputText:key:modifiers:client:` 或 `handleEvent:client:` 三種方式。選一套 delivery path，依 core 的 consumed 結果回傳，不讓同一按鍵重複進 core。 |
| Marked text / commit | Client 的 `IMKTextInput.setMarkedText(_:selectionRange:replacementRange:)` 與 `insertText(_:replacementRange:)`。Selection range 相對於 marked string；有效 replacement range 相對於 client document。依 SDK 契約使用 `NSNotFound` 表示目前插入位置，不讀取完整文件。 |
| Text units | `NSString`/`NSRange` 使用 UTF-16 code units；adapter 必須從 core UTF-8 byte offsets 明確換算，不能使用 Swift `String.count` 代替。 |
| Lifecycle | `activateServer:`、`deactivateServer:`、`commitComposition:` 與 `inputControllerWillClose`。依 input spec 處理 reset/commit，不因重複 callback 再次提交文字。 |
| Native candidates | `IMKCandidates.init(server:panelType:)`、`updateCandidates`（Swift `update()`）、`show:`、`hide`；controller 的 `candidates:`、`candidateSelectionChanged:`、`candidateSelected:`。Selection callbacks 必須對應有效 context 與候選快照。 |
| Candidate keys | `setSelectionKeys:` 接受 virtual key codes，不是 Unicode digits；SDK 預設使用 1–9，且候選面板預設先收 key event。須核對 `IMKCandidatesSendServerKeyEventFirst` 與 core 的選字模式，避免 Dachen tone keys 被面板搶先處理。 |

已通過的 SDK probe：用 default CLT SDK 執行 `xcrun swiftc -typecheck`，從 stdin 匯入 `AppKit`/`InputMethodKit`，typecheck `IMKTextInput.setMarkedText`、`insertText`、`IMKCandidates(server:panelType:)`、`update()` 與 `hide()`；module cache 放在 repository 外的 temporary directory，exit status **0**。沒有建立 server、呼叫輸入 API、link bundle 或執行 native UI。

### Build prerequisites and commands

- 需要一致 architecture 的 Rust `ime-ffi` static library、C header、Swift host 與 macOS SDK。
  - `crates/ime-ffi/include/bopomofo.h` 與 linked library 必須來自同一 ABI revision，並核對 `BPM_ABI_VERSION`/`bpm_abi_version()`。
- `.app` bundle 須具備 executable、Info.plist 與核准的 lexicon。
  - 查核 `InputMethodConnectionName` 與 server connection name，以及 `InputMethodServerControllerClass` 與實際 Swift/Objective-C class name 一致。
  - 採用本專案獨立 identifier，不覆寫其他 input source。
- 目前 build script/Info.plist 設定的 deployment target **13.0** 只是 build 設定，不是已驗證的最低 OS。SDK API 歷史 availability 同樣不能作為 native 支援證據。

以下是 repository 的 build 重播指令。Bundle 與 standalone contracts 已在本機及 macOS arm64 CI 通過，仍不代表 installed native IME 通過：

```bash
cargo build --release -p ime-ffi
bash platforms/macos/scripts/build.sh
plutil -lint 'platforms/macos/build/Bopomofo Development.app/Contents/Info.plist'
file 'platforms/macos/build/Bopomofo Development.app/Contents/MacOS/BopomofoDevelopment'
otool -L 'platforms/macos/build/Bopomofo Development.app/Contents/MacOS/BopomofoDevelopment'
```

記錄 bundle checksum、architecture 與 SDK。此處不提供自動註冊或啟用的驗證捷徑；安裝、啟用、簽章、notarization 與發布仍需各自的授權及證據。

## Windows: Text Services Framework

### Official API investigation

已以 `curl` 閱讀 Microsoft Learn 的 [TSF overview](https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework) 及下列 API 文件。Windows SDK 使用 `msctf.h` 的 TSF interfaces；Rust 的 `windows` crate 對應 `Win32_UI_TextServices` 等 bindings。Bindings 不會提供 MSVC linker、Windows loader 或 native 測試環境。

| 功能 | 具體 API 與 adapter 要求 |
| --- | --- |
| COM activation | [`ITfTextInputProcessorEx`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itftextinputprocessorex) 的 `ActivateEx`，以及 `ITfTextInputProcessor` 的 `Activate`/`Deactivate`。管理 COM lifetime、client ID、thread/document/context 與 sinks；不能共用 composition state。 |
| Key sink | [`ITfKeystrokeMgr`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfkeystrokemgr) 的 `AdviseKeyEventSink`/`UnadviseKeyEventSink`；[`ITfKeyEventSink`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfkeyeventsink) 的 test/key callbacks。 |
| Read-only key probe | [`OnTestKeyDown`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeyeventsink-ontestkeydown) 只判斷 `pfEaten`。本專案的 test callbacks 必須使用不改狀態的 core probe；真正的 `OnKeyDown` 才處理事件。 |
| Text access | [`ITfContext::RequestEditSession`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession) → [`ITfEditSession::DoEditSession`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfeditsession)。文字變更要有適當 read/write edit cookie；檢查 method HRESULT **及** `phrSession`。`TF_ES_SYNC` 可能失敗，不可在任意 callback 假設可取得同步 write lock。 |
| Composition / commit | [`ITfContextComposition::StartComposition`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition)、`ITfComposition`/composition sink、[`ITfInsertAtSelection::InsertTextAtSelection`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinsertatselection-inserttextatselection)、[`ITfComposition::EndComposition`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcomposition-endcomposition)。以 UTF-16/`WCHAR` 與有效 ranges 更新文字；focus 改變後不得讓延遲 edit session 提交到新 context。 |
| Candidate presentation | [`ITfUIElementMgr`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfuielementmgr) 的 Begin/Update/End，及 [`ITfCandidateListUIElement`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcandidatelistuielement)/[`ITfCandidateListUIElementBehavior`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcandidatelistuielementbehavior)。這些 interfaces 不是會自動畫出的 candidate window。 |
| UI ownership | [`BeginUIElement`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfuielementmgr-beginuielement) 的 `pbShow=false` 表示 application 可自行畫 UI，TIP 不得再顯示自身 UI。須分開驗證 host-drawn 與 TIP-owned native presentation。 |
| Disabled contexts | 查核 [predefined compartments](https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-compartments)，包括 `GUID_COMPARTMENT_KEYBOARD_DISABLED`、`GUID_COMPARTMENT_EMPTYCONTEXT` 與 keyboard open/close；不繞過 host 限制。 |
| Registration boundary | [`ITfInputProcessorProfiles`](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfinputprocessorprofiles) 的 Register/AddLanguageProfile 等會改變系統狀態；此調查沒有呼叫。DLL/COM class、profile、category 的安裝與移除須另經授權。 |

Microsoft API 頁面的歷史 minimum-client metadata 不代表本專案支援該 Windows 版本。**最低 Windows 版本與已驗證 SDK version 尚未選定。**

### Build prerequisites and commands

需要 Windows native runner、[Windows SDK](https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/)、MSVC C++ build tools/linker、相應 target/import libraries，以及 Rust MSVC target。每一 architecture 分別建置 DLL；in-process COM DLL 必須符合 application architecture。

以下是 Windows native developer shell 指令，不是 macOS 執行結果。三架構 native build／非註冊 tests 已獨立通過，installed desktop 仍未驗證：

```powershell
cargo build --release -p ime-windows --target x86_64-pc-windows-msvc
cargo build -p ime-windows --target x86_64-pc-windows-msvc
cargo test -p ime-windows --target x86_64-pc-windows-msvc
dumpbin /headers target/x86_64-pc-windows-msvc/release/bopomofo_tsf.dll
dumpbin /exports target/x86_64-pc-windows-msvc/release/bopomofo_tsf.dll
Get-FileHash target/x86_64-pc-windows-msvc/release/bopomofo_tsf.dll -Algorithm SHA256
```

另在相應環境對 `i686-pc-windows-msvc`、`aarch64-pc-windows-msvc` 重複驗證。檢查 machine type、dependencies 與所需 COM exports；不能把 macOS 上 target-gated unit tests 或 DLL cross-compilation 當成 Windows TSF 驗證。本文件不執行 `regsvr32`、profile registration 或 system setting 變更。

## Linux: Fcitx5

### Official API investigation

已以 `curl` 閱讀 Fcitx 官方 [addon tutorial](https://fcitx-im.org/wiki/Develop_an_simple_input_method) 及其引用的 [InputMethodEngineV2 API](https://codedocs.xyz/fcitx/fcitx5/classfcitx_1_1InputMethodEngineV2.html)、[InputContext API](https://codedocs.xyz/fcitx/fcitx5/classfcitx_1_1InputContext.html)、[CommonCandidateList API](https://codedocs.xyz/fcitx/fcitx5/classfcitx_1_1CommonCandidateList.html)。另核對官方 **5.1.14** tag 的 headers，固定 commit 為 [`7688bd73249f4fc880baaa520c2240a2579dc0b9`](https://github.com/fcitx/fcitx5/tree/7688bd73249f4fc880baaa520c2240a2579dc0b9)。這是 API 調查版本，不是已驗證最低 runtime version。

| 功能 | Pinned SDK API 與 adapter 要求 |
| --- | --- |
| Addon factory / engine | `AddonFactory`、`FCITX_ADDON_FACTORY`、[`InputMethodEngineV2`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx/inputmethodengine.h)；`keyEvent`、`activate`、`deactivate`、`reset` 轉成 core events，不另實作 parser/decoder。 |
| Context ownership | 透過 `InputContextManager::registerProperty` 與 `InputContextProperty`/`FactoryFor` 保存每個 input context 的 engine handle；context 關閉時釋放，composition 不共用。 |
| Key consumption | `KeyEvent::filterAndAccept()` 只用於 core 已 consumed 的 events。Release 與 shortcuts 依 core 契約透傳，不記錄 key values。 |
| Preedit / commit | [`InputContext`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx/inputcontext.h) 的 `inputPanel()`、`updatePreedit()`、`updateUserInterface(UserInterfaceComponent::InputPanel)` 與 `commitString()`。 |
| Text / candidates | [`InputPanel`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx/inputpanel.h) 的 `setClientPreedit`/`setPreedit`、`setCandidateList`；[`CommonCandidateList`/`CandidateWord::select`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx/candidatelist.h)。Candidate UI 由 Fcitx5 處理；callback 須拒絕 stale snapshot。 |
| Cursor units | [`Text::setCursor`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx/text.h) 明定 byte position。須使用合法 UTF-8 boundary，不誤轉為 UTF-16 或 Unicode scalar count。 |
| Host capabilities | [`CapabilityFlag`](https://github.com/fcitx/fcitx5/blob/7688bd73249f4fc880baaa520c2240a2579dc0b9/src/lib/fcitx-utils/capabilityflags.h) 包括 `Preedit`、`Password`、`Sensitive`；查核 context capability/狀態，不繞過敏感欄位限制。Client preedit 不可用時依 Fcitx 的 input panel 機制呈現。 |

官方 tutorial 含 key logging、install 與 restart 的示例；**不得直接照用**。本專案不記錄輸入內容，本次也不執行 install、restart 或 input method configuration。

### Build prerequisites and commands

- Linux C++17 compiler、CMake、Ninja 或 Make、pkg-config、Fcitx5 Core/Config/Utils development files，以及相同 architecture 的 Rust library/C ABI header。
- 官方 tutorial 使用 `find_package(Fcitx5Core REQUIRED)`、`Fcitx5::Core`，並示範 CMake 3.21 與 C++17。
- Debian/Ubuntu 的 development package names 可包括 `build-essential`、`cmake`、`ninja-build`、`pkg-config`、`libfcitx5core-dev`、`libfcitx5config-dev`、`libfcitx5utils-dev`、`libxkbcommon-dev`。目前 adapter 的 JSON bridge 另需 `nlohmann-json3-dev`。這些是環境前提，**本次沒有安裝**。
- Header 與 Rust library 須來自一致 ABI revision；C++ 取得的 Rust buffers 只用配對的 Rust free API 釋放。
- 官方 addon layout 是 `lib/fcitx5/<addon>.so`、`share/fcitx5/addon/<addon>.conf` 與 `share/fcitx5/inputmethod/<inputmethod>.conf`。路徑須由實際 Fcitx install prefix/libdir 決定；此處不寫入任何 prefix。

Rust／C++ build 流程已在 Linux x64／arm64 headless CI 通過。以下指令在 `target/fcitx5` 建置，不安裝或啟用 addon：

```bash
pkg-config --modversion Fcitx5Core
cargo build --release -p ime-ffi
cmake -S platforms/linux/fcitx5 -B target/fcitx5 -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build target/fcitx5
ctest --test-dir target/fcitx5 --output-on-failure
find target/fcitx5 -name '*.so' -print
```

Headless container 可以提供 build/link 或 C ABI test 證據，但不能證明 addon 已在 Fcitx5 desktop 正確載入。未經另行授權不執行 `cmake --install`、修改 Fcitx profile、重啟 daemon 或改變 toolkit environment settings。

## Native validation handoff

以下全部是 **unverified**，不是已通過結果。後續結果須保存於 `docs/native-validation.md`；該文件與 native test scripts 由實作階段建立。本文件不代替它們。

| 環境 | 必須獨立取得的證據 |
| --- | --- |
| macOS arm64 / native text app、browser、Electron | 實際 marked text、cursor/range、候選翻頁與選取、commit 一次、取消、shortcut 透傳、focus 切換、input source 切換、多螢幕與 scaling。 |
| macOS x86_64 | 相同項目須由相應 architecture 獨立驗證；不能沿用 arm64 native 結果。 |
| Windows x64、x86、ARM64 / 相應 application types | Native DLL build/load、COM lifetime、test callback 不改狀態、edit-session lock/failure、composition、candidate UI ownership、focus/cancel、registration/removal 與復原。 |
| Linux X11 / GTK、Qt、browser、Electron | Addon build/load、client/panel preedit、candidate selection、commit/cancel、shortcut、focus、移除與復原；每個組合分別記錄。 |
| Linux Wayland / GTK、Qt、browser、Electron | 同上；依實際 compositor、frontend protocol、toolkit module/version 分別記錄，不由 X11 結果推論。 |
| 每個平台的 Unicode / lifecycle cases | 非 BMP Han character、emoji、combining sequence、stale callbacks、重複 reset、context isolation、不完整音節與 64-syllable limit 的平台呈現。 |

每筆 native 結果至少記錄 OS/build、CPU architecture、app/toolkit/compositor version、repository revision、artifact SHA-256、lexicon revision、測試步驟、預期與實際結果，以及 `passed`、`failed` 或 `unverified`。不得記錄使用者實際輸入內容；驗證只使用固定的自製 fixture。

仍受阻的交接：各平台 installed IME／native application matrix、Windows 實際 document edit sessions、Linux X11／Wayland desktop sessions，以及安裝與 desktop 操作授權。Linux headless development build 不再是缺少環境的項目。最低 OS、CPU/app 支援聲明、signing/notarization 與 release approval 都尚未完成。
