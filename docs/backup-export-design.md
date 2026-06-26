# 学習データ バックアップ（エクスポート/インポート）設計

## 1. 目的とスコープ

アプリ更新時の再インストールで学習進捗が失われないよう、**全データを 1 ファイルにエクスポート / インポート（マージ復元）** できるようにする。Windows・Android 両対応。

対象テーブル（[0001_initial.sql](../src-tauri/migrations/0001_initial.sql)）:

| テーブル | 内容 | 既存 import |
|----------|------|:-----------:|
| `decks` | デッキ設定 | ○（upsert 済） |
| `cards` | カード内容 | ○（upsert 済） |
| `srs_records` | **学習進捗（FSRS 状態）** | ✗ 未対応 |
| `review_logs` | **復習履歴** | ✗ 未対応 |
| `app_state` | テーマ等のアプリ設定 | ✗（バックアップ対象外とする） |

> 既存の deck import は意図的に srs を維持し新規 srs を作らない（[import.rs:48](../src-tauri/src/commands/import.rs#L48)）。本機能は **進捗を含めた** 別系統のバックアップとして追加する。

### 決定事項（合意済み）
- **形式**: zip/JSON ダンプ（既存 zip 形式の拡張）
- **復元**: マージ（既存に統合。同一行は上書き、無い行は追加）
- **着手**: 本実装まで完了（§9 参照）。Android 実機 UX 確認のみ残。

## 2. バックアップファイル形式

**デッキ単位の自己完結ユニット**を1つの zip に内包する。各デッキフォルダがそれ単体で完結（内容＋進捗）するため、全体バックアップ・選択復元・将来のデッキ単位エクスポートを同一形式で扱える。ファイル拡張子は `.akbak`（中身は zip）または `.zip`。

```
ankiros_backup_YYYYMMDD.zip
├── backup.json                       # バックアップ全体のメタ（新規）
└── decks/
    ├── <deck_id_A>/                  # ← デッキ単位の自己完結ユニット
    │   ├── deck.json                 # DeckJson 形式（既存 deck import と互換）
    │   ├── cards.json                # Card 配列（user_notes・timestamps 含む = 完全忠実）
    │   ├── srs.json                  # SrsRecord 配列（学習進捗）
    │   └── logs.json                 # ReviewLog 配列（復習履歴）
    └── <deck_id_B>/
        └── ...
```

`backup.json`:
```json
{ "schema_version": "1", "exported_at": "2026-06-26T12:00:00Z", "deck_ids": ["..."] }
```

### この形式の利点
- **全体バックアップ** = 全ユニットを束ねた 1 ファイル（取りこぼしゼロ・一貫性確保）。
- **選択復元** = インポート UI で「このデッキだけ戻す」をユニット単位で選べる（ファイルは 1 つのまま粒度を出せる）。
- **将来のデッキ単位エクスポート** = 1 ユニットを切り出すだけ。`deck.json` は既存のデッキ zip と同形式（互換）。`cards.json` は user_notes・timestamps まで含む**完全忠実版**（バックアップは内容配布用の CardJson より優先で忠実性を取る）。内容配布用に共有する場合は、将来 `notes` 形式（CardJson）へ落として書き出せばよい。

> 既存の `extract_zip()`（[import.rs:314](../src-tauri/src/commands/import.rs#L314)）は単一デッキ内容 zip 用のため、バックアップは `decks/<deck_id>/` 配下を走査する**専用エクストラクタ**を新設する（zip slip 対策の `enclosed_name()` 等は流用）。`srs.json` / `logs.json` が無いユニットは「内容のみ」として従来どおり取り込めるよう後方互換にする。

## 3. バックエンド（Rust）

### 3.1 追加モデル（models.rs）
- `ReviewLog`（`review_logs` 行に対応、現状モデル無し）— Serialize + Deserialize + Type
- `SrsRecord` に `Deserialize` を追加（現状 Serialize のみ）
- `BackupMeta`（schema_version / exported_at / deck_ids）

### 3.2 追加コマンド（新規 `commands/backup.rs`）

```rust
// 全データを zip バイト列にして返す
#[tauri::command] async fn export_backup(db) -> AppResult<Vec<u8>>

// zip バイト列をマージ復元（content:// 対応のため bytes 受け）
#[tauri::command] async fn import_backup(db, data: Vec<u8>) -> AppResult<BackupImportResult>
```

- `export_backup`: 各デッキについて decks/cards/srs/logs を SELECT → デッキごとに `deck.json` / `cards.json` / `srs.json` / `logs.json` を作り、`decks/<deck_id>/` 配下へ配置 → `zip::ZipWriter`（既存テストで `ZipWriter` 使用実績あり [import.rs:535](../src-tauri/src/commands/import.rs#L535)）でメモリ上に組み立て、`Vec<u8>` を返す。
- `import_backup`: `decks/<deck_id>/` ユニットを走査する専用エクストラクタで展開 → **1 トランザクション**内で各ユニットを順に upsert（選択復元時は対象 deck_id のユニットのみ）:
  - decks → 既存 `upsert_deck` 流用
  - cards → 既存 `upsert_card` 流用（※ user_notes 維持の挙動に注意。バックアップ復元では user_notes も復元したいので、cards upsert に user_notes を含める分岐 or 専用関数を用意）
  - srs_records → `INSERT ... ON CONFLICT(card_id,deck_id,mode) DO UPDATE`
  - review_logs → `INSERT ... ON CONFLICT(id) DO NOTHING`（id は UUID。履歴は重複追加しない）
- マージ方針:既存行は上書き、未知行は追加。外部キー順（decks→cards→srs/logs）に投入。

### 3.3 コマンド登録 / バインディング
- `commands/mod.rs` に `pub mod backup; pub use backup::*;`
- lib.rs の invoke_handler / specta collect に追加 → `npm run gen:bindings` で [bindings.ts](../src/bindings.ts) 再生成。

## 4. フロントエンド（React）

### 4.1 エクスポート
```ts
const bytes = await call(commands.exportBackup());     // number[]
const blob = new Uint8Array(bytes);
const path = await save({ defaultPath: "ankiros_backup.zip",
                          filters: [{ name: "バックアップ", extensions: ["zip"] }] });
if (path) await writeFile(path, blob);
```
- Windows: `save()` + `writeFile()` で完結（容易）。
- **Android（要検証ポイント）**: `dialog.save()` は SAF の「ドキュメント作成」を起動し content:// を返す。`writeFile()` がそこへ書ける必要がある。実機で要確認。NG の場合のフォールバック:
  - `@tauri-apps/plugin-fs` で `BaseDirectory.Download` 等の固定先へ保存し、保存パスをトースト表示
  - もしくは share intent で「他アプリへ送る」
- 読み込み（インポート）は既存の `open()`＋`readFile()` パターンを流用 → Android 解決済み（[Decks.tsx:51-59](../src/routes/Decks.tsx#L51-L59)）。

### 4.2 インポート
```ts
const file = await open({ filters: [{ name: "バックアップ", extensions: ["zip"] }] });
const bytes = await readFile(file);
const res = await call(commands.importBackup(Array.from(bytes), null)); // 第2引数=復元対象deck_ids（null=全件）
// res: { decks, cards_created, cards_updated, srs_imported, logs_imported }
```

### 4.3 UI 配置
設定/ホーム画面あたりに「バックアップ」セクション（エクスポート・インポートボタン）を追加。`isAndroid()`（[platform.ts](../src/lib/platform.ts)）で保存経路の出し分けが要る場合のみ分岐。

## 5. capabilities / 権限（確認済み）
プラグインのソースで確認した結果:
- `save` は **`dialog:default` に既に含まれる**（`allow-message` / `allow-save` / `allow-open`）→ 追加不要。
- `writeFile` は `fs:default`（read 系のみ）に**含まれない**ため、**`fs:allow-write-file` を追加**した（[default.json](../src-tauri/capabilities/default.json)）。
- 選択したファイル/URI のパススコープは dialog プラグインが付与する（既存の `open()`＋`readFile()` インポートが動く仕組みと同じ）。
- このケイパビリティは `platforms` 未指定のため **Android にも適用**される。

## 6. スキーマバージョニング
- `backup.json.schema_version` を検証（既存 deck.json と同じく "1" 固定から開始、[import.rs:154](../src-tauri/src/commands/import.rs#L154) と同方針）。
- 将来のスキーマ変更時はインポート側でバージョン分岐し変換。

## 7. 検証手順（実装後）
1. **単体（Rust）**: temp DB に decks/cards/srs/logs を投入 → export → 別 temp DB に import → 件数・内容一致を assert（既存 [import.rs テスト](../src-tauri/src/commands/import.rs#L588) と同型）。
2. **Windows 実機**: エクスポート→ファイル生成確認→学習を進める→インポート→マージ結果確認。
3. **Android 実機**: ★ `save()`/`writeFile()` の content:// 書き込み確認（最優先の不確実点）→ エクスポート/インポート往復→ **APK 再インストール（同一署名）後にインポートで復元** できることを確認。

## 8. 難易度サマリ
| 項目 | 難易度 | 不確実性 |
|------|--------|----------|
| バックエンド export/import | 低〜中 | 低（既存コード流用） |
| バインディング/コマンド登録 | 低 | 低 |
| フロント読み込み | 低 | 無（実装済パターン） |
| フロント書き出し（Windows） | 低 | 低 |
| **フロント書き出し（Android）** | 中 | **中**（SAF 保存の実機検証） |

主要リスクは Android の「保存」経路のみ。読み込みは解決済みのため、全体としては現実的に実装可能。

## 9. 実装状況（2026-06-26）

**Windows 上で実装・ビルド・テスト完了。Android 実機での UX 確認のみ残**。

実装済み:
- バックエンド [commands/backup.rs](../src-tauri/src/commands/backup.rs): `export_backup` / `import_backup`（マージ・選択復元対応・1トランザクション）。
- モデル [models.rs](../src-tauri/src/models.rs): `ReviewLog` / `BackupImportResult` 追加、`SrsRecord` に Deserialize、`DeckJson` 系に Serialize。
- コマンド登録（[mod.rs](../src-tauri/src/commands/mod.rs) / [lib.rs](../src-tauri/src/lib.rs)）、バインディング再生成（[bindings.ts](../src/bindings.ts) に `exportBackup` / `importBackup`）。
- ケイパビリティ `fs:allow-write-file` 追加。
- フロント [lib/backup.ts](../src/lib/backup.ts) ＋ [Settings.tsx](../src/routes/Settings.tsx) に「データのバックアップ」セクション（エクスポート/インポート）。
- Rust 単体テスト 3 件（往復で進捗・user_notes 復元 / ログ冪等 / 選択復元）→ 全 12 件 green。`npm run build`（tsc + vite）green。

実装上の確定事項:
- `cards.json` は CardJson ではなく **完全な `Card` モデル**で出力（user_notes・timestamps を忠実に復元するため）。`deck.json` は DeckJson 互換のまま。
- フロントは save/read とも単一経路でプラットフォーム分岐不要（プラグインが content:// 差を吸収）。

### Android 実機チェックリスト（次トリップ）
1. 設定→「エクスポート」→ SAF の保存ダイアログが出てファイル名が編集でき、保存先に zip ができること。
   - もし `.zip` フィルタで保存しにくい/拡張子が付かない場合は `ZIP_FILTER` を緩める（MIME `application/octet-stream` 等）。
2. 「インポート」→ その zip を選び、件数メッセージが出てデッキ・進捗が復元されること。
3. もし書き込みが scope/permission エラーになる場合のみ、fs スコープ設定を追加（dialog 付与で足りる想定だが保険）。
4. **本命の回帰確認**: 実機で学習を進める → エクスポート → APK 再インストール（同一署名で進捗が残るのが前提だが、署名変更/アンインストール時の保護として）→ インポートで復元できること。
