# Agent 工作規則

## 溝通與文件

- 先說明最重要的資訊。
- 刪除不必要或重複的內容。
- 使用清楚的結構、常用字詞與短句。
- 中文技術文件使用簡明的正體中文，並參考 Simplified Technical English 的寫作原則。
  - 使用明確的動詞與一致的用語。
  - 每句只說明一個重點。
- 英文技術文件遵循 ASD-STE100（Simplified Technical English）。
  - 使用標準核准的詞彙與寫作規則。
  - 保留必要的技術名稱。
- 專有名稱使用英文名稱。
- 技術名詞使用英文。
- 先說明主要概念，再補充必要細節。
- 確保內容正確。
- 規則必須明確且可驗證。
- 相依規則使用縮排子項目，放在主規則下。
- 圖表使用 Mermaid 語法。
  - 說明架構、流程、互動或狀態變化時，若圖表能幫助理解，則將圖表加入文件。
- 只使用技術性文字。
- 直接表達。

## 程式碼風格

- 遵循 KISS（Keep It Simple），使用符合需求的最簡單方案。
- 遵循 YAGNI（You Aren't Gonna Need It），不為假設的未來需求新增功能。
  - 不為這類需求新增 configuration option、environment variable 或 feature flag。
- 每個 source file 只負責一個主要用途。
- 變更不得超出使用者要求的範圍。
- 只有符合下列至少一項條件時，才拆分不同職責的程式碼。
  - 拆分後更容易理解。
  - 拆分後更容易測試。
  - 修改一項職責時，不需修改其他職責。
- 不得將緊密相依的邏輯分散到多個檔案。

## 變更邊界

- 修改 core 介面或平台 adapter 前，先閱讀 `docs/architecture.md`。
- 輸入規則放在共用的 Rust core。
- OS 整合放在平台 adapter。
- 自行實作輸入引擎，不使用 `chewing`、`librime` 或其他現成輸入引擎。
- 輸入處理必須離線。
- 不得記錄或傳送 keystrokes、preedit text 或 committed text。
- 未經使用者明確授權，不得更換專案授權條款。
- 匯入或散布 lexicon data 前，先確認資料來源與授權。
- 執行下列動作前，必須取得使用者明確授權。
  - 安裝、註冊、啟用或移除 native IME。
  - 修改系統設定。
  - 發布 artifact。

## 測試

- 執行與變更元件相關的檢查。
- 分別回報失敗、受阻與未執行的檢查。
- 變更輸入行為時，新增 replay regression case。
- Core tests、platform builds 與 native IME checks 必須分別提供證據。
- 沒有對應的 native validation 結果，不得宣稱支援某個 OS、CPU 架構或應用程式組合。
