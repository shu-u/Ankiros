# 単語帳アプリ (Ankiros)

中国語学習を主用途とした FSRS スペーシング・リピティション単語帳デスクトップアプリ。
Write & Compare 方式の能動的想起訓練を採用。仕様書 v1.2 準拠。

## 技術スタック

- **アプリ:** Tauri v2 + React + Vite + TypeScript
- **IPC型生成:** tauri-specta（`src/bindings.ts` を自動生成）
- **ルーター:** TanStack Router / **状態管理:** Zustand
- **UI:** shadcn/ui 風コンポーネント + Tailwind CSS
- **差分表示:** `diff` パッケージ
- **DB:** SQLite（sqlx + sqlx migrate）
- **SRS:** FSRS（`rs-fsrs` クレート）

## セットアップ

```bash
npm install
```

### 社内ネットワークでのセットアップ（重要）

社内 npm プロキシ（`prod.supplyhold.net`）は TLS 傍受用の社内CA証明書を提示します。
Node 標準ではこのCAを信頼できず `UNABLE_TO_GET_ISSUER_CERT_LOCALLY` で `npm install` が失敗します。

本リポジトリの `.npmrc` は、Windows 証明書ストアから書き出した CA バンドル
（`C:\Users\<ユーザー>\.certs\corp-npm-ca.pem`）を `cafile` で参照する設定になっています。
証明書ストアが更新された場合は、以下で CA バンドルを再生成してください（PowerShell）:

```powershell
$dir = "$env:USERPROFILE\.certs"; New-Item -ItemType Directory -Force $dir | Out-Null
$out = "$dir\corp-npm-ca.pem"
$certs = @()
$certs += Get-ChildItem Cert:\LocalMachine\Root
$certs += Get-ChildItem Cert:\LocalMachine\CA
$certs += Get-ChildItem Cert:\CurrentUser\Root
$sb = New-Object System.Text.StringBuilder
foreach ($c in $certs) {
  $b = [Convert]::ToBase64String($c.RawData, 'InsertLineBreaks')
  [void]$sb.AppendLine("-----BEGIN CERTIFICATE-----"); [void]$sb.AppendLine($b); [void]$sb.AppendLine("-----END CERTIFICATE-----")
}
[IO.File]::WriteAllText($out, $sb.ToString())
```

（一時的に回避するだけなら環境変数 `NODE_EXTRA_CA_CERTS` に上記PEMのパスを設定しても可。）

## 開発実行

```bash
npm run tauri dev
```

初回起動時に `app_data/app.db` を作成し、マイグレーションを自動適用します。

## IPC バインディングの再生成

Rust 側のコマンド／型を変更したら、TypeScript バインディングを再生成します:

```bash
npm run gen:bindings   # = cd src-tauri && cargo run --bin gen_bindings
```

（`tauri dev` 実行時にも debug ビルドで自動生成されます。）

## ビルド

```bash
npm run tauri build
```

## 動作確認用サンプルデッキ

`sample_decks/hsk3/` にサンプルデッキを同梱しています。インポート方法は 2 通り:

- **ZIP から取り込み**（クロスプラットフォーム）: 「デッキ一覧」→「ZIPから取り込み」で
  `sample_decks/hsk3.zip` を選択。
- **フォルダから取り込み**（デスクトップ）: 「デッキ一覧」→「フォルダから取り込み」で
  `sample_decks/hsk3` フォルダを選択。

ZIP は現行のフォルダ構造（`deck.json` ＋ `cards/*.json`）をそのまま固めたもので、
`Compress-Archive -Path sample_decks\hsk3\* -DestinationPath sample_decks\hsk3.zip` で再生成できます。
Android 対応の設計・引き継ぎ事項は [docs/android-port-design.md](docs/android-port-design.md) を参照。

## データ構造

- **カードコンテンツ:** JSON ファイル（AIが生成・人間が編集）→ インポート時に SQLite へ取り込み
- **学習データ・設定:** SQLite（アプリが管理）

JSON フォーマットの詳細は仕様書 §4 を参照。AIへのカード生成依頼テンプレートは仕様書 §15。

## プロジェクト構成

```
src/                      フロントエンド
├── bindings.ts           tauri-specta 自動生成（編集不可）
├── lib/                  api ラッパー・ユーティリティ
├── store/                Zustand ストア (appStore / sessionStore)
├── components/           UI プリミティブ・共通部品
└── routes/               8画面 (Home/Decks/DeckDetail/Study/Result/Cards/CardDetail/Settings)

src-tauri/
├── migrations/           sqlx マイグレーション
└── src/
    ├── commands/         IPC コマンド (decks/cards/import/session/stats/state)
    ├── models.rs         型定義 (specta::Type)
    ├── db.rs             プール初期化・行マッピング
    ├── srs.rs            FSRS マッピング (spec §5.5)
    └── lib.rs            Builder 構築・ウィンドウ状態復元
```
