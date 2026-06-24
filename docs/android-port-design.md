# Android 対応 設計ドキュメント

> ステータス: **ドラフト（設計検討中）** / 最終更新: 2026-06-24
> このドキュメントは Windows 11 専用だった Ankiros を Android でも動かすための
> 設計判断・タスク・確定仕様を一元管理するための生きた文書です。決定が変わったら
> ここを更新してください。

---

## 1. ゴールとスコープ

- **ゴール**: 既存の機能（デッキ管理・FSRS 学習・Write & Compare 想起訓練・統計）を
  Android スマートフォンでも利用できるようにする。
- **非ゴール（今回やらないこと）**:
  - サーバー / Backend API の新設（本アプリは**ローカル完結型**を維持する）。
  - **自動クラウド同期**（行単位マージには同期サーバーが必要で方針に反する。個人利用のため
    不要。端末移行/保険は「手動バックアップ」で代替する。§8 参照）。
  - iOS 対応（Tauri 2 では技術的に同経路だが、今回の対象外）。
- **対象端末/配布**: 開発者個人の Android 端末でのみ使用。配布は**署名済み APK を生成**できれば足りる
  （ストア公開・CI 配布は不要）。

---

## 2. 現状アーキテクチャ（移植が容易な理由）

| レイヤ | 技術 | Android 移植性 |
|---|---|---|
| シェル | **Tauri 2** | ◎ Android/iOS を**公式サポート** |
| フロント | React 19 + Vite + TS + Tailwind + TanStack Router + Zustand | ◎ Android WebView でそのまま動作 |
| ロジック | Rust（FSRS=`rs-fsrs`, `chrono`, `uuid`） | ◎ 純粋 Rust、NDK 向けにコンパイル可 |
| DB | SQLite（`sqlx` + migrate） | ○ NDK ビルドで動作。パスは `app_data_dir()` が OS 別に解決 |
| IPC 型生成 | tauri-specta | ◎ 影響なし |

**結論: Backend を含む大規模な破壊的変更は不要。** サーバーは存在せず、データは端末内
SQLite に保存される（[lib.rs:151-153](../src-tauri/src/lib.rs#L151-L153)）。本質的な作業は
(A) UI のモバイル対応 と (B) 取り込み方式の再設計 の 2 点に集中する。

---

## 3. 作業領域の一覧

| # | 領域 | 内容 | コスト | 備考 |
|---|---|---|---|---|
| A | UI レスポンシブ化 | 固定サイドバー [Layout.tsx:14](../src/components/Layout.tsx#L14)、`max-w-5xl p-8`、固定ウィンドウ 1200×800 → スマホ向けにボトムナビ・余白・タッチ対応 | 中 | ロジック非依存の純 UI 作業 |
| B | **取り込み再設計** | フォルダ取り込み（`std::fs::read_dir`）が Android のスコープドストレージで不可 → **ZIP 単一ファイル方式**へ（§5） | 中 | 今回の中心。下記で確定仕様化 |
| C | ウィンドウ状態の保存/復元 | `save/restore_window_state`（[lib.rs:16-87](../src-tauri/src/lib.rs#L16-L87)）はデスクトップ専用 → `#[cfg(desktop)]` で除外 | 小 | 数行 |
| D | Android ビルド環境 | Android SDK / NDK / JDK、`tauri android init`、署名鍵 | 小（一回限り） | CI も別途検討 |
| E | プラグイン確認 | `tauri-plugin-opener` / `tauri-plugin-dialog` の Android 動作確認 | 小 | dialog は単一ファイル選択を使用（§5） |

---

## 4. インポートの現状仕様（再掲・確定済み）

- **デッキ = フォルダ**。`deck.json`（メタ＋設定）＋ `cards/batch_*.json`（カード本体）。
  実例: [sample_decks/hsk3/](../sample_decks/hsk3/)。
- 取り込みコマンドは 2 種類（[lib.rs:101-102](../src-tauri/src/lib.rs#L101-L102)）:
  - `import_deck_folder(folder_path)` … デッキまるごと（[import.rs:134](../src-tauri/src/commands/import.rs#L134)）
  - `import_cards_folder(deck_id, folder_path)` … 既存デッキへカード追加（[import.rs:228](../src-tauri/src/commands/import.rs#L228)）
- カードは **`id` で upsert**。既存カードはコンテンツのみ上書きし、`user_notes` と
  SRS 記録は保持（[import.rs:46-67](../src-tauri/src/commands/import.rs#L46-L67)）。
- **バッチファイル 1 つ = 1 トランザクション**（[import.rs:113-122](../src-tauri/src/commands/import.rs#L113-L122)）。
- `deck.json` の `schema_version` は `"1"` 固定検証（[import.rs:153](../src-tauri/src/commands/import.rs#L153)）。
- 関連スキル `build_deck` / `verify_cards` がこの**フォルダ＋バッチ構造**前提で動作。

### JSON スキーマ（現行 / 変更しない）

```jsonc
// deck.json  (models.rs: DeckJson)
{
  "schema_version": "1",
  "deck_id": "hsk3",            // 英数字・_ のみ
  "name": "HSK3 単語",
  "description": "…",           // 省略可
  "language": "zh",             // 省略時 "zh"
  "settings": {
    "test_modes": ["recognition", "pronunciation"],  // 必須・非空
    "daily_new_limit": 20,      // 省略時 20
    "daily_review_limit": 100,  // 省略時 100
    "fsrs": { "target_retention": 0.9, "max_interval_days": 365 }
  }
}
```

```jsonc
// cards/batch_001.json  (models.rs: CardJson[]) — 配列
[
  {
    "id": "hsk3_0001",
    "hanzi": "…", "pinyin_accepted": ["…"], "meaning": "…",
    "example_sentences": [], "synonyms": [], "antonyms": [], "tags": [],
    "notes": "…"                // → DB の ai_notes へマッピング
  }
]
```

---

## 5. 確定方針: ZIP 単一ファイル取り込み

### 5.1 概要

デッキを **1 つの ZIP ファイルにパッケージ**し、ユーザーは**単一ファイル選択ダイアログ**で
それを選んで取り込む。両 OS（Windows / Android）で同一の仕組みが使え、選んだファイルの
中身（バイト列）を確実に読めるため、プラットフォーム固有コードがほぼ不要。

### 5.2 ファイル形式

- **拡張子は普通の `.zip`**（独自拡張子は使わない）。種別は**中身を見て判定**する（§5.3）。
- **ZIP 内のディレクトリ構造は現行フォルダ構造をそのまま維持**:
  ```
  my_deck.zip
  ├── deck.json          ← これがあればデッキまるごと取り込み
  └── cards/
      ├── batch_001.json
      └── batch_002.json
  ```
  → これにより JSON スキーマもバッチ分割も**一切変更不要**、`build_deck`/`verify_cards`
    スキルは従来通りフォルダを生成し、最後に zip 化するだけで対応できる。
- **中身による種別判定ルール**:
  - ルートに `deck.json` がある → **デッキまるごと取り込み**（デッキ作成/更新 ＋ カード upsert）。
  - `deck.json` が無く `cards/*.json`（または直下の `*.json`）のみ → **カードのみ取り込み**
    （取り込み先デッキを UI で選択させてから追加）。
  - どちらにも該当しない（`deck.json` も カード JSON も無い）→ エラー表示。
- ルート直下の単一フォルダで包まれている zip（例: `my_deck/deck.json`）も許容できるよう、
  ZIP 展開後に `deck.json` の位置を探索してベースパスを決める（堅牢化のため）。

### 5.3 取り込みフロー

1. フロント: `@tauri-apps/plugin-dialog` の `open()` を**単一ファイル選択**（`directory:false`,
   フィルタ = `zip`）で呼ぶ。選択結果のパス（またはバイト列）を Rust へ渡す。
2. Rust: ZIP を展開し、**メモリ上または一時ディレクトリ**でエントリを列挙。
   - 既存の `collect_card_files` 相当を「ZIP エントリ列挙」に置き換える。
   - **中身を見て種別判定**（§5.2）: `deck.json` の有無でデッキ取り込み / カードのみ取り込みを分岐。
   - `deck.json` の検証・`upsert_card`・バッチ単位トランザクションは**そのまま再利用**。
3. 結果は現行と同じ `ImportResult { created, updated }` を返す。
4. カードのみ取り込みの場合は、手順1の前または後に取り込み先デッキ（`deck_id`）を UI で選択させる。

### 5.4 既存ロジックの再利用範囲

| 処理 | 変更 |
|---|---|
| ファイル一覧の取得元 | `read_dir` → **ZIP エントリ列挙**（ここだけ変更） |
| `deck.json` 検証 / `schema_version` | 変更なし |
| `upsert_card`（id 上書き、user_notes/SRS 保持） | 変更なし |
| バッチ = トランザクション | 変更なし（ZIP エントリ単位） |
| 返り値 `ImportResult` | 変更なし |

### 5.5 デスクトップとの併存

- **Windows ではフォルダ取り込みも残す**（`#[cfg(desktop)]` で従来コマンドを併存）。
  現状の開発ワークフロー（sample_decks をフォルダ選択）は無傷。
- Android では ZIP 取り込みのみ提供。
- フロントは実行プラットフォームを判定し、提示する取り込み UI を出し分ける。

### 5.6 検討した代替案（却下）

| 案 | 却下理由 |
|---|---|
| 単一の巨大 JSON | バッチ分割が崩れ、スキルの検証単位と不一致。大規模デッキで肥大化 |
| Android で SAF フォルダ取り込み | `content://` ツリーを扱う Android 専用プラグインが必要で最も高コスト |
| URL / クラウド取り込み | サーバーが必要になり「ローカル完結」を崩す |

### 5.7 必要な依存・実装メモ

- Rust: ZIP 展開クレート（例: `zip`）を追加。NDK ターゲットでのビルド確認が必要。
- ファイル選択は `directory:false` のため SAF の単一ドキュメント選択で content URI が
  バイト列として読める（フォルダツリーの問題は発生しない）。

---

## 6. レイアウト設計（モバイル対応 / 領域 A）

### 6.1 基本方針

- **同一の React アプリをレスポンシブCSSで出し分ける**（モバイル専用アプリは作らない）。
- ブレークポイントは Tailwind の **`md`（768px）** で分岐。
  - `< 768px`（スマホ）: ボトムタブバー
  - `≥ 768px`（デスクトップ）: 現状のサイドバー（現行UIを維持）
- **実機なしで検証可能**: フロントは Web 技術のため、`npm run dev` をブラウザの狭い幅
  （DevTools のデバイスエミュレーション 375px 等）で開けばモバイルレイアウトを確認できる。
  実機が要るのは Android ビルド / ZIP 取り込み(SAF) / NDK のみ。

### 6.2 ナビゲーション（確定: ボトムタブバー）

- スマホでは画面下部に**固定ボトムタブ**（ホーム / デッキ / 設定 の3項目、アイコン＋ラベル）。
  親指で届き、Android で最も一般的。遷移先が3つだけなので最適。
- デスクトップでは従来のサイドバー（`hidden md:flex`）。ボトムタブは `md:hidden`。
- 実装: [Layout.tsx](../src/components/Layout.tsx) にサイドバーとボトムタブを両方描画し、
  Tailwind の表示クラスで出し分ける。固定ボトムタブと重ならないよう、コンテンツに
  下パディング（`pb-24 md:pb-8`）を付与。
- スマホでは最上部にアプリ名のスリムなヘッダーを表示（任意・ブランド表示／ステータスバー余白）。

### 6.3 各画面のモバイル調整

- **コンテンツ余白**: `p-8` → `p-4 md:p-8`。
- **Study 画面**（[Study.tsx](../src/routes/Study.tsx)、最重要）:
  - キーボード前提をタッチ主役へ。`(Enter)` / `1〜4` / `（Esc）` 等の**キーヒントはモバイルで非表示**
    （`hidden md:inline`）。キーボードショートカット自体は残す（物理キー接続時や PC で有効）。
  - 評価ボタン（Again/Hard/Good/Easy）は4列のまま、**タッチ高さ 44px 以上**を確保。
  - 入力欄のオートフォーカスでソフトキーボードが出る挙動はそのまま活かす。
- **Home / Decks など**: 既に `grid-cols-1 sm:grid-cols-2` 等でほぼ対応済み。幅を狭めれば成立。

### 6.4 未確定・要検討

- ソフトキーボード表示時のビューポート縮小（入力欄が隠れないか）の実機確認。
- セーフエリア（Android のジェスチャーバー）とボトムタブの余白調整（`env(safe-area-inset-bottom)`）。

### 6.5 実装状況（2026-06-24 時点）

**UI レスポンシブ（領域 A）— 完了**
- [x] [Layout.tsx](../src/components/Layout.tsx): サイドバー(`md+`)／ボトムタブ(`<md`)の出し分け、
      スリムなモバイルヘッダー、セーフエリア余白を実装。
- [x] [Study.tsx](../src/routes/Study.tsx): キーヒントをモバイル非表示、評価ボタン/めくるボタンの
      タッチ高さ確保、問題カード余白を `p-6 md:p-8`。ショートカット自体は維持。
- [x] [Cards.tsx](../src/routes/Cards.tsx): 行の横溢れ対策（意味を `truncate`、`min-w-0`）、
      検索入力をモバイル全幅。
- [x] [CardDetail.tsx](../src/routes/CardDetail.tsx): 余白 `p-6 md:p-8`。
- [x] Home / Decks / DeckDetail / Result / Settings / Modal: 既存の対応で変更不要と確認。

**ウィンドウ状態の除外（領域 C）— 完了**
- [x] [lib.rs](../src-tauri/src/lib.rs): `save/restore_window_state` と `on_window_event` を
      `#[cfg(desktop)]` で囲み Android ビルドから除外。デスクトップは従来通り。

**ZIP 取り込み（領域 B）— Windows 実装＆テスト完了 / Android 統合のみ残**
- [x] [import.rs](../src-tauri/src/commands/import.rs): `zip` クレート追加、`extract_zip()` と
      共通化した `import_card_batches()` / `upsert_deck()` を実装。コマンド `import_deck_zip` /
      `import_cards_zip` を追加（中身判定 §5.2）。フォルダ取り込みも併存。
- [x] Rust ユニットテスト 5 件（zip 中身判定・cards/優先・包みフォルダ・カードのみ・temp DB への
      取り込み/再取り込み）が `cargo test` で全通過。**中核ロジックは Windows で検証済み**。
- [x] バインディング再生成（`commands.importDeckZip` / `importCardsZip`）。
- [x] フロント: [Decks.tsx](../src/routes/Decks.tsx) に「ZIPから取り込み」、
      [DeckDetail.tsx](../src/routes/DeckDetail.tsx) に「ZIPでカード追加」ボタンを追加。
- [x] テスト用サンプル: [sample_decks/hsk3.zip](../sample_decks/hsk3.zip) を生成。
- [ ] **Android 統合（実機・§10.2 で詳述）**: Android では `open()` が `content://` URI を返すため
      `std::fs::read(zip_path)` では読めない。読み取り部分の差し替えが必要（中核ロジックは流用可）。

**検証**
- [x] `cargo test`（Rust 5 件）成功、`npm run build`（tsc + vite）成功、`npm run dev` 正常配信。

> ブラウザ確認: `npm run dev` → http://localhost:1420 → DevTools(F12) →
> デバイスツールバー(Ctrl+Shift+M) で幅 375px 等。広げるとサイドバーへ戻る。
> ZIP 取り込み確認: デッキ一覧→「ZIPから取り込み」→ `sample_decks/hsk3.zip` を選択。

---

## 7. 想定フェーズ（実装順の案）

> ✅ = 実機なしで完了済み / ⏳ = Android 環境（D/E）が必要

- ✅ **領域 A — UI モバイル最適化**: レスポンシブ化・ボトムナビ・タッチ操作（§6.5）。
- ✅ **領域 C — ウィンドウ状態の `#[cfg(desktop)]` 除外**（§6.5）。
- ✅ **領域 B — ZIP 取り込み（中核）**: Rust 実装＋ユニットテスト＋フロント UI、Windows で検証済み（§6.5）。
- ⏳ **フェーズ 0 — ビルド基盤（D）**: `tauri android init`、NDK/JDK 整備、`tauri android dev` 起動確認（§10）。
- ⏳ **フェーズ 1 — 最小動作版（E）**: 表示・学習・統計が実機で動くことを確認。
- ⏳ **フェーズ 2 — ZIP 取り込みの Android 統合（E）**: `content://` URI の読み取り対応（§10.2）。
- ⏳ **フェーズ 3 — 実機 UI 検証（E）**: ソフトキーボード・セーフエリア・タッチ感、フォルダ取り込みボタンの非表示。
- ⏳ **フェーズ 4 — 仕上げ（D）**: アイコン、署名、**APK ビルド**（§10.5）。任意で手動バックアップ。

---

## 8. 未決事項 / 今後の検討

- [x] 拡張子 → **`.zip` で確定**、中身判定（§5.2）。
- [x] 配布方法 → **署名済み APK の個人利用のみで確定**（ストア/CI 不要）。
- [x] ZIP 展開はメモリ上か一時ディレクトリか → **メモリ上で確定**（`extract_zip(&[u8])`、一時展開なし）。
- [ ] **ZIP 取り込みの Android 統合（`content://` 対応）** → 対応方針は §10.2 に確定。実機で実装・検証。
- [ ] `build_deck` 出力（フォルダ）を zip 化する手順の整備。Windows なら
      `Compress-Archive -Path <deck>\* -DestinationPath <deck>.zip`（[sample_decks/hsk3.zip](../sample_decks/hsk3.zip) 生成済み）。
- [ ] Android の「他アプリから開く / 共有シート」での `.zip` 受け取り（intent-filter）。任意。
- [ ] **手動バックアップ**（SQLite のエクスポート/インポート）を入れるか。自動クラウド同期は非ゴール（§1）。
- [ ] 署名鍵（keystore）の生成・保管方法。
- [ ] 音声（`audio_path`）の扱い。現状 nullable で未使用に見えるが Android でのパス解決を要確認。

---

## 9. 参照

- 既存仕様書 v1.2（リポジトリ外）— JSON フォーマット詳細は §4、AI 生成テンプレートは §15。
- 関連スキル: `build_deck`, `verify_cards`, `verify_deck`。
- 主要コード: [import.rs](../src-tauri/src/commands/import.rs), [lib.rs](../src-tauri/src/lib.rs),
  [Decks.tsx](../src/routes/Decks.tsx), [Layout.tsx](../src/components/Layout.tsx)。

---

## 10. 付録: D/E 引き継ぎ手順（Android 環境構築後に行う作業）

> ここから先は **Android 実機/エミュレータと SDK 環境が必要** で、開発者本人が後で実施する想定。
> 領域 A（UI）・C（cfg 除外）・B の中核（ZIP ロジック）は実機なしで実装・検証済み（§6.5）。
> **残りは「環境構築（D）」と「実機での統合・検証（E）」のみ**。

### 10.0 残作業チェックリスト（全体像）

- [ ] **D-1** Android ビルド環境（JDK / SDK / NDK / Rust ターゲット）を整える（§10.1）
- [ ] **D-2** `tauri android init` で Gradle プロジェクト生成（§10.3）
- [ ] **E-1** `tauri android dev` で起動確認、表示・学習・統計が動くか（§10.4）
- [ ] **E-2** ⚠️ **ZIP 取り込みの `content://` URI 対応**（最重要・§10.2）
- [ ] **E-3** フォルダ取り込みボタンを Android で非表示にする（§10.2 末尾）
- [ ] **E-4** 実機 UI 検証: ソフトキーボード・セーフエリア・タッチ操作（§6.4）
- [ ] **E-5** dialog の zip フィルタ（拡張子→MIME）と capability 権限が Android で効くか確認（§10.2）
- [ ] **D-3** 署名済み（or debug 署名）APK をビルド（§10.5）

### 10.1 必要なもの（一度だけ）

1. **JDK 17**（Android Studio 同梱の JBR でも可）。`JAVA_HOME` を設定。
2. **Android SDK + NDK**（Android Studio の SDK Manager から導入）。環境変数:
   - `ANDROID_HOME`（SDK パス。例 `C:\Users\<user>\AppData\Local\Android\Sdk`）
   - `NDK_HOME`（例 `%ANDROID_HOME%\ndk\<version>`）
3. **Rust の Android ターゲット**:
   ```powershell
   rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```
   （実機の多くは `aarch64`。エミュレータは `x86_64`。）

### 10.2 ⚠️ ZIP 取り込みの Android 対応（E-2 / 最重要）

**問題**: デスクトップでは `open()` ダイアログがファイルの**実パス**を返すため、Rust 側の
`import_deck_zip` / `import_cards_zip` は `std::fs::read(zip_path)` でそのまま読める。
しかし **Android（SAF）では `open()` が `content://...` の URI を返す**ため、`std::fs::read`
では読めず失敗する。**ここだけが ZIP 取り込みで唯一 Android 特有の対応点。**

**幸い中核ロジックはバイト列ベース**で書いてある（[import.rs](../src-tauri/src/commands/import.rs)
の `extract_zip(bytes: &[u8])` 以降）。差し替えが必要なのは「URI からバイト列を得る」入口だけ。

推奨対応（いずれか）:
1. **フロントでバイト列を読み、コマンドへ渡す**（クロスプラットフォームで最も素直）
   - `@tauri-apps/plugin-fs` を依存追加し、`readFile(path)` で `Uint8Array` を取得
     （plugin-fs は Android の `content://` 読み取りに対応）。
   - Rust に `import_deck_zip_bytes(data: Vec<u8>)` / `import_cards_zip_bytes(deck_id, data)`
     を追加（中身は `extract_zip(&data)` 以降を流用、`std::fs::read` を省くだけ）。
   - フロントは `open()` → `readFile()` → `commands.importDeckZipBytes(Array.from(bytes))`。
   - これならデスクトップでも同じ経路で動く（パスからも読める）ので、最終的に
     `std::fs::read` 版を廃止して一本化してもよい。
2. Rust 側で URI を解決する方法もあるが、1 の方がプラグインに乗れて確実。

**capability / 権限（E-5）**: [capabilities/default.json](../src-tauri/capabilities/default.json) に
`dialog:default` がある。plugin-fs を使う場合は `fs:default`（または必要な read 権限）を追加。
`"windows": ["main"]` の指定が Android でも有効か、`tauri android init` 後の mobile スキーマで確認。

**dialog の拡張子フィルタ（E-5）**: `filters: [{ extensions: ["zip"] }]` は Android では MIME
（`application/zip`）にマップされる。実機で `.zip` が選べるか確認。選びにくい場合はフィルタを
緩める。

### 10.2.1 フォルダ取り込みを Android で隠す（E-3）

- 現状デスクトップ用に「フォルダから取り込み」ボタンを残してある（[Decks.tsx](../src/routes/Decks.tsx)
  /[DeckDetail.tsx](../src/routes/DeckDetail.tsx)）。Android では SAF フォルダ選択が機能しないため、
  これらのボタンを隠す。`@tauri-apps/plugin-os` の `platform()` で `"android"` を判定して出し分けるのが簡単。

### 10.2.2 その他コードの前提（確認のみ）

- 領域 C（ウィンドウ状態の `#[cfg(desktop)]` 除外）は**実装済み**（§6.5）。
- `tauri.conf.json` の `app.windows`（1200×800 等）は **Android では無視**されるので変更不要。
- アプリ識別子は既に `com.p984172.ankiros`（[tauri.conf.json](../src-tauri/tauri.conf.json#L5)）で設定済み。
- DB パスは `app_data_dir()` 解決のため Android でも自動で正しい場所になる（[lib.rs](../src-tauri/src/lib.rs#L153)）。

### 10.3 Android プロジェクトの初期化（一度だけ）

```powershell
npm run tauri android init
```
→ `src-tauri/gen/android`（Gradle プロジェクト）が生成される。

### 10.4 実機/エミュレータで動作確認（任意）

```powershell
npm run tauri android dev
```

### 10.5 APK のビルド

- **個人利用なら debug 署名 APK が最も簡単**（鍵管理不要・そのまま端末にインストール可）:
  ```powershell
  npm run tauri android build --apk --debug
  ```
- リリース署名 APK を作る場合は keystore が必要:
  ```powershell
  keytool -genkey -v -keystore ankiros.keystore -alias ankiros -keyalg RSA -keysize 2048 -validity 10000
  ```
  → `src-tauri/gen/android` の signing 設定（`keystore.properties` / Gradle）に登録してから:
  ```powershell
  npm run tauri android build --apk
  ```
  （`--apk` を付けないと既定で AAB が出力される。特定アーキのみは `--target aarch64` 等。）

### 10.6 出力先とインストール

- APK は `src-tauri/gen/android/app/build/outputs/apk/...` に出力される。
- 端末へ: USB 接続で `adb install <path>.apk`、またはファイルを端末へ転送して
  「提供元不明のアプリ」を許可してインストール。
