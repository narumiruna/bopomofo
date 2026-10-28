# Lexicon sources

## Status and policy

正式 Traditional Chinese Zhuyin lexicon 與 phrase frequency data **尚未選定或核准匯入**。目前只允許使用本專案自製的最小 fixtures。外部來源的調查結果不等於 import 或 release approval。

本次於 **2026-10-28 UTC** 以 `curl` 查閱來源、license 與資料格式。供檢查的下載檔只放在 repository 外的 temporary directory；未將外部 lexicon、corpus、importer 或輸入引擎程式碼加入 repository。

- 保留根目錄 `LICENSE` 的 GNU AGPL v3 policy，不更換專案授權。
  - 自製 fixtures 依 repository 的 AGPL policy 管理，不另加新的 license。
  - 外部資料保留各自的 license 與 notices；不得因為放入 AGPL repository 就宣稱其已改以 AGPL 授權。
- 自行實作 parser、index 與 decoder。不匯入 McBopomofo、`chewing`、`librime` 或其他輸入引擎的實作。
- 來源下載與查核不在 runtime 進行。Keystroke processing 完全離線，不下載資料，也不記錄或傳送輸入內容。
- Import 前須核對**資料本身**及上游來源的權利。Engine license、公開下載、GitHub license badge 或 checksum 都不能單獨證明資料的轉換與散布權利。

## Source register

| ID | 可提供的內容 | 條款查核結果 | 本專案狀態 |
| --- | --- | --- | --- |
| `fixture-local` | 自製字、詞、讀音及人工設定的 ranking scores | 不取自外部 dictionary 或 corpus；依現有 AGPL policy 管理 | 允許用於開發；不是產品詞庫或真實詞頻證據。 |
| `unicode-unihan-17` | Character readings；少量 character/pronunciation frequencies | Unicode License V3 明確允許修改與散布，須保留 notice | **Candidate，未核准匯入**；可供 character fallback 評估，不足以取代 phrase lexicon。 |
| `cc-cedict-snapshot` | Traditional/Simplified forms、phrase Pinyin、English definitions | 資料 header 與 publisher 均指定 CC BY-SA 4.0 | **Candidate，未選用**；需處理 ShareAlike、讀音品質與 snapshot 保存，沒有 phrase frequency。 |
| `mcbopomofo-data` | 直接的 Zhuyin 字詞 mapping 與 phrase occurrence counts | Root MIT 與 data 的上游 provenance 必須分開查核 | **Blocked approval**；libtabe notices 與 frequency provenance 尚未確認完整。 |
| `moe-revised` | Traditional Chinese 字詞、Zhuyin、definitions | 官方指定 CC BY-ND 3.0 Taiwan；未取得本專案轉換所需的另行許可 | **Blocked for transformed lexicon**；不匯入、不假設 ND 允許 extraction。 |

## Self-authored fixtures

開發用資料位於 `fixtures/`，由實作 fixture 的變更建立並保存版本。不得從下列 candidates 複製 entries 來冒充自製資料。

- 每個 entry 的字詞、讀音及 score 必須由本專案自行編寫。
- Scores 是用來固定 regression ordering 的人工值，不稱為實測 corpus frequency。
- 固定開發版本為 `fixture-local-v1`，路徑為 `fixtures/lexicon.tsv`，共 39 筆，696 bytes。
  - 原始檔 SHA-256：`c54ced5e1e44472337f9bb5c5b8fe41e7a692a33dbfa8d360991704888f11dcd`。
  - `ime-cli build-lexicon` 正規化產物 SHA-256：`7f57031c86bb4c3d909b4e24a08e54d771bba4cc7d06a7c164e121fb62be37d6`。
  - 兩次匯出結果以 `cmp` 驗證完全相同；版本由包含本檔與 fixture 的 repository revision 固定。
- 極小 fixture 的測試不能證明正式詞庫的 coverage、候選品質或代表性效能。

## Unicode Unihan 17.0.0

這是本次找到的**明確允許資料修改與散布**的來源，但仍是未選用的 candidate。

| 欄位 | 查核紀錄 |
| --- | --- |
| Publisher | Unicode, Inc. |
| 固定版本 | Unicode **17.0.0**；archive 內的 data headers 亦標示此版本。 |
| Data URL | <https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip> |
| Release record | <https://www.unicode.org/Public/17.0.0/ucd/ReadMe.txt> |
| 固定格式文件 | [UAX #38 revision 39, Unicode 17.0.0](https://www.unicode.org/reports/tr38/tr38-39.html)；不以會更新的 latest 文件代替版本紀錄。 |
| 相關檔案 | `Unihan_Readings.txt`；`Unihan_Variants.txt` 可供 variant 關係查核。 |
| License | [Unicode License V3](https://www.unicode.org/license.txt)，SPDX `Unicode-3.0`。 |
| 授權適用範圍 | [Unicode Terms of Use](https://www.unicode.org/terms_of_use.html) 將 `/Public/` 下的 computer data files 納入 Unicode Data Files，並指定 License V3，除非另有註明。這不代表可用相同條款散布整份 Unicode specification。 |
| Attribution | 保留 archive 內 `© 2025 Unicode®, Inc.` notice，並在 data 或 associated documentation 包含完整 copyright and permission notice。遵守不得以權利人名稱宣傳背書的限制。 |
| Conversion rights | License 明列 use、copy、modify、merge、publish、distribute、sell。可據此評估 Pinyin-to-Zhuyin conversion、欄位選取與 index 建置；須保留 notices，且仍需本專案核准與品質驗證。 |
| Checksum availability | 已實際下載並計算下列 SHA-256。檢查過的 release directory 未列出對應 publisher checksum 或 signature；這是本地取得內容的識別值，不是 publisher signature。 |

```text
Unihan.zip (8,518,517 bytes)
SHA-256 f7a48b2b545acfaa77b2d607ae28747404ce02baefee16396c5d2d7a8ef34b5e

license.txt (2026-10-28 查閱的 Unicode License V3)
SHA-256 e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96
```

資料限制：

- [`kMandarin`](https://www.unicode.org/reports/tr38/tr38-39.html#kMandarin) 是最常用的 Pinyin reading，不是完整 heteronym 或 phrase reading 表。
  - 兩個值時，第二個偏向 `zh-Hant (TW)`；一個值時適用於兩者。
  - 此 property 主要用於 collation/transliteration，不保證每筆符合現行 Taiwan dictionary 的偏好。
- 需要自行實作並測試 Pinyin-to-Zhuyin conversion。此處未匯入 conversion code 或生成的讀音。
- [`kHanyuPinlu`](https://www.unicode.org/reports/tr38/tr38-39.html#kHanyuPinlu) 提供 character/pronunciation frequencies，源於 Modern Standard Beijing Chinese Frequency Dictionary 相關資料；不是 Taiwan phrase counts。
- 不得將 variant 關係當成一對一的 Simplified-to-Traditional conversion，或把 character frequency 直接宣稱為 phrase frequency。
- 17.0.0 archive 不含 `kFrequency` property；不得依舊版欄位名稱假設它存在。

## CC-CEDICT

這是允許改作的 phrase dictionary candidate，不是 permissive license，也不是 frequency source。

| 欄位 | 查核紀錄 |
| --- | --- |
| Publisher / contributors | CC-CEDICT contributors；MDBG。Archive 另保留 CEDICT attribution：Copyright (C) 1997, 1998 Paul Andrew Denisowski。 |
| 官方來源與條款 | <https://www.mdbg.net/chinese/dictionary?page=cc-cedict> |
| Data URL | <https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.txt.gz> |
| 已檢查的 snapshot | Header：`version=1`、`subversion=0`、`format=ts`、`charset=UTF-8`、`entries=125326`、`date=2026-10-27T01:55:52Z`、`time=1793066152`。 |
| License | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en)；資料 header 明列此 license 的 URL。 |
| Attribution / conversion | 條款允許 share/adapt，包括商業使用；須保留所提供的 attribution、license link 與適用 notices，標示修改，並遵守 adaptation 的 ShareAlike 條件。不能直接宣稱轉換後資料只需 AGPL notice。 |
| 可評估的轉換 | 選取 Traditional forms、轉換 Pinyin、去除 definitions、重新排序；若產物構成 adaptation，須依 ShareAlike 條件散布。實際產物及與 AGPL code 的分離方式尚未審定。 |
| Checksum / pin 限制 | 已取得下列 snapshot checksum；官方下載 URL 會更新，不能只靠 URL 重建這一版。正式選用前須安排有權保存及再散布的固定 snapshot。未發現此下載頁提供 publisher checksum。 |

```text
cedict_1_0_ts_utf-8_mdbg.txt.gz
SHA-256 347499200f7eb34a922993fc71397d4033845096a8ca070b760ec200ff9be2ed

解壓後 cedict_1_0_ts_utf-8_mdbg.txt
SHA-256 bbc4b27f26bee1a262f4d7939667a040c7857c7fc0200a2fa7a86d3352a09eae
```

它包含 Traditional forms，但 Pinyin 不等於逐詞驗證過的 Taiwan Zhuyin reading。Erhua、非標準 syllables、proper nouns 與多讀音須另行驗證。它沒有可直接用於 segmentation 的 phrase frequency 欄位；本次未選用或合併此資料。

## McBopomofo data

**不能以 engine 的 MIT license 當成所有 data 的完整權利證據。** 以下只調查 data 與 notices，未匯入或執行 engine、build scripts 或 importer。

- 固定 repository revision：[`18e2060dd5d0b511753ad79a8fe0d112e944f1af`](https://github.com/openvanilla/McBopomofo/tree/18e2060dd5d0b511753ad79a8fe0d112e944f1af)，commit date `2026-10-26T22:45:14Z`。
- [Pinned data README](https://github.com/openvanilla/McBopomofo/blob/18e2060dd5d0b511753ad79a8fe0d112e944f1af/Source/Data/README.md) 說明：
  - `BPMFBase.txt` 是 single-character Zhuyin mapping。
  - `BPMFMappings.txt` 是 phrase mapping，原始內容由 libtabe `tsi.src` 簡化並修改，README 稱其為 BSD Licensed。
  - `phrase.occ` 是 phrase occurrence data，不是 engine code。
- [Pinned root LICENSE.txt](https://github.com/openvanilla/McBopomofo/blob/18e2060dd5d0b511753ad79a8fe0d112e944f1af/LICENSE.txt) 是 MIT，copyright `2011-2026 Mengjuei Hsieh et al.`。此 notice 應保留，但不能用它省略 libtabe 或其他資料來源的 notices。
- [Pinned textpool.rc](https://github.com/openvanilla/McBopomofo/blob/18e2060dd5d0b511753ad79a8fe0d112e944f1af/Source/Data/textpool.rc) 指向 `~/textpool.11262021`。它沒有提供可重建的公開 corpus archive、corpus license 或其 checksum。

下列 SHA-256 是本次對 pinned raw files 實際計算的結果，不是上游提供的簽章：

| Pinned file URL | SHA-256 |
| --- | --- |
| [BPMFBase.txt](https://raw.githubusercontent.com/openvanilla/McBopomofo/18e2060dd5d0b511753ad79a8fe0d112e944f1af/Source/Data/BPMFBase.txt) | `46e64a53dc9c6baa2cbe6e0b1af572f8ef6aafb50597178b5fd53df804316ce7` |
| [BPMFMappings.txt](https://raw.githubusercontent.com/openvanilla/McBopomofo/18e2060dd5d0b511753ad79a8fe0d112e944f1af/Source/Data/BPMFMappings.txt) | `8dd751310c5effb7f9bdb24da494cb308492acd73a1665974fd886c4caf1fb26` |
| [phrase.occ](https://raw.githubusercontent.com/openvanilla/McBopomofo/18e2060dd5d0b511753ad79a8fe0d112e944f1af/Source/Data/phrase.occ) | `f3b52d572233ea2c0f2eac1872adfa2fbe2491d8ad051f875863217707561a0e` |
| [LICENSE.txt](https://raw.githubusercontent.com/openvanilla/McBopomofo/18e2060dd5d0b511753ad79a8fe0d112e944f1af/LICENSE.txt) | `72ed32193b0c629def66df63791b2fb125946b593a3251380e6b949488f47b72` |

尚缺的證據：

- 確認選用 entries 的完整 data license chain，取得 libtabe 原始 copyright/permission notices 與後續新增來源的 notices。
- 確認 `BPMFBase.txt` 與 `phrase.occ` 的權利範圍，不從 root MIT 或 counts 的格式推論全部上游授權已解決。
- 確認 occurrence data 的產生方式、可再散布的授權證據與是否能重建。未公開 corpus 不必然表示 counts 禁止散布，但目前也不足以證明本專案有權使用及散布。
- 在上述證據完整前，**不核准 extraction、normalization、合併、生成 artifact 或散布**。Checksum 已固定不會解除此限制。

## Ministry of Education dictionary

| 欄位 | 查核紀錄 |
| --- | --- |
| Publisher / attribution | Ministry of Education, Taiwan；MOE Revised Mandarin Chinese Dictionary。正式散布還須依官方使用說明核對完整 attribution。 |
| 官方 license page | <https://language.moe.gov.tw/001/Upload/Files/site_content/M0001/respub/index.html> |
| 官方 download/version page | <https://language.moe.gov.tw/001/Upload/Files/site_content/M0001/respub/dict_reviseddict_download.html> |
| 查閱時列出的版本 | `dict_revised_2015_20260929`；頁面更新日期 `115-10-1`。 |
| 具名 archive URL | <https://language.moe.gov.tw/001/Upload/Files/site_content/M0001/respub/download/dict_revised_2015_20260929.zip> |
| License | [CC BY-ND 3.0 Taiwan legal code](https://creativecommons.org/licenses/by-nd/3.0/tw/legalcode)。官方明示允許重製、散布及傳輸，包括商業用途，但不得修改，並須遵照使用說明。 |
| Conversion rights | Legal code section 3 允許為不同媒體/格式所必需的技術修改，除此之外不授予製作 adaptation 的權利。不能據此直接斷言抽取詞音、刪除 definitions、讀音正規化或合併成 IME lexicon 都是允許的純格式轉換。 |
| Checksum availability | 本次未下載 archive；沒有 archive checksum。檢查過的 download page 未提供 checksum。具名 URL 不代表內容已驗證或權利已核准。 |

此來源不是 permissive data。未取得適用於本專案轉換的另行許可或明確的權利判定前，不匯入或散布轉換產物。第三方 JSON mirror 的程式碼 license 也不能消除原資料的 ND 限制。

## Additional data-rights audit

### libtabe tsi.src

固定來源為 [libtabe 0.2.6-1.2 data directory](https://sources.debian.org/src/libtabe/0.2.6-1.2/tsi-src/)，另與 [kcwu/libtabe revision ea382a829c916f2cceda2022afdf8e8471e6e7b5](https://github.com/kcwu/libtabe/tree/ea382a829c916f2cceda2022afdf8e8471e6e7b5/tsi-src) 核對。

本次於 repository 外檢查資料，未建立可散布的詞庫產物，也未匯入 engine code。

| File | 本地計算的 SHA-256 |
| --- | --- |
| tsi.src | `5889b0960229dfd3cb33f7f6f8f3f4b9e54b631825796951d30bbef04590e78f` |
| tsi-src/COPYING | `efe0869f6626da81c78537da381117fe64cfe24ac50d2f2b702d90d6d9a81bd4` |
| tsi-src/README | `6dcc58ba7e68dabf282481dd3e2557b4bebe1376292bab2bf4994e70c209473c` |

Data-directory README 明確指定該目錄適用 COPYING。此版本包含 TaBE／Pai-Hsiang Hsiao 與 CCL/IIS/Academia Sinica 的 BSD-3-Clause-style grants，允許 covered material 的修改與散布，並要求保留 notices。

但完整來源權利鏈尚未確認：

- README 指出超過 95% vocabulary 來自 Chih-Hao Tsai，另有未能找到原始 reference 的 IOME／Xcin contributions。
- [Tsai publisher archive](https://technology.chtsai.org/wordlist/tsaiword.zip) 的 README.TXT，日期為 1996-04-13，寫明「Feel free to use this list for any non-commercial purposes.」
  - README.TXT SHA-256：`6ea926da1b9ed71d71392fd7c84d43ee37f0956f81f59d7a8a4d66493ae8af14`。
- 尚未取得後續 BSD relicensing 或適用於本專案的完整授權證據。
- 舊條款不證明後來的 BSD 發布一定無效，但不足以宣稱整條權利鏈已確認。
- 所需外部證據是 archived grant 或 rights-holder confirmation，不是一般的使用者批准。

README 記載 counts 由 1998 年收集的 Taiwan web pages 抽樣 1,200,000 頁，在 1999 年以 maximum matching 統計，且已取得統計許可。這是歷史語料，不代表現代輸入分布。原始 corpus 未公開，不妨礙固定 count file 的確定性轉換，但不能據此宣稱語料可完整重建。

格式檢查發現 CP950 encoding、139,189 rows、零頻率、多讀音 alternatives 與少量重複。來源的零 counts 不符合目前正整數 schema，且有讀音須審查。權利問題未解決前，不進行正式匯入，也不靜默修正資料。

### wordfreq

檢查版本為 [rspeer/wordfreq revision 912caf64b657478d1dff1138efdc078947d54bb1](https://github.com/rspeer/wordfreq/tree/912caf64b657478d1dff1138efdc078947d54bb1)。

- LICENSE.txt 指定 Apache-2.0 僅適用 code，並將 data 條件交由 NOTICE.md。
- NOTICE.md 指定 data 的 CC BY-SA 4.0 與個別 attribution 條件。
- README 指出 Chinese data 包含 provenance 不明的 Jieba wordlist，並將 Traditional／Simplified 合併成內部的「Oversimplified Chinese」。
- README 另拒絕將 data 單獨轉成 CSV；本次不自行判定這段文字與 CC BY-SA 的適用關係。
- 缺少本專案可確認的 Traditional Chinese reading／frequency 轉換與完整來源證據，因此未選用，未下載或散布頻率資料。

已查閱的固定文件 SHA-256：

- README.md：`0f1db058e1df2e6bda2db5d0ea851a7111d2390ec16a9939d971ef53ea24f684`。
- LICENSE.txt：`8990c551671a51765c4dfe9428f3e1d58dc327f49aca346a32d28cc0fcc7aa88`。
- NOTICE.md：`d9b104dafe63886af55e7cea7484824e32e12d4e1f57f319923dd643b36d6b15`。
- 不以 code license 推論 data 的完整權利。

## Import and release gate

每次核准外部資料前，必須保存以下可查核紀錄：

1. Publisher、原始 URL、固定 release/revision、實際下載內容的 SHA-256，以及 checksum 是 publisher 提供或本地計算。
2. 適用於所選 data files 的完整 license、copyright、上游 provenance 與必要 attribution。若有例外許可，保存其範圍及證據。
3. 明確列出 extraction、reading conversion、filtering、deduplication、scoring、合併及 redistribution 各步驟的權利依據。未證明的權利保持 blocked。
4. 固定 importer revision、format version、轉換規則、穩定排序與輸出 checksum。保留 phrase 與 syllable sequence 的明確對應。
5. 驗證 syllables、tone、重複 entries、缺失 reading 及無效 score。不得靜默為 phrase 猜測讀音，或把不同來源的 counts 當成可直接比較的 frequency。
6. 保存 data notices，並查核產物的 license obligations 與現有 AGPL policy。取得 release 授權後才可發布。

目前仍 blocked：可供產品使用且權利完整的 Traditional Chinese phrase lexicon、適合 Taiwan 使用情境的 phrase frequency、完整 notice bundle，以及可重建的正式 data artifact。Core 原型可用自製 fixture 繼續；產品品質、代表性 benchmark 與正式資料散布驗收不能據此通過。
