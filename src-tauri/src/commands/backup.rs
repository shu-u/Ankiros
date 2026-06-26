//! 学習データのバックアップ（エクスポート/インポート）。
//!
//! 形式: zip。デッキ単位の自己完結ユニットを内包する（docs/backup-export-design.md §2）。
//! ```
//! ankiros_backup_*.zip
//! ├── backup.json                       # メタ（schema_version / exported_at / deck_ids）
//! └── decks/<deck_id>/
//!     ├── deck.json   # DeckJson 形式（既存 deck import と互換）
//!     ├── cards.json  # Card 配列（user_notes・timestamps 含む = 完全忠実）
//!     ├── srs.json    # SrsRecord 配列（学習進捗）
//!     └── logs.json   # ReviewLog 配列（復習履歴）
//! ```
//! インポートはマージ（同一行は上書き、無い行は追加、履歴は重複させない）。

use crate::db::{card_from_row, srs_from_row, vec_to_json, Db};
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::util::now_rfc3339;
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};

const BACKUP_SCHEMA_VERSION: &str = "1";

/// backup.json のメタ情報。
#[derive(Debug, Serialize, Deserialize)]
struct BackupMeta {
    schema_version: String,
    exported_at: String,
    deck_ids: Vec<String>,
}

// ============================================================
// 行 → モデル
// ============================================================

fn review_log_from_row(row: &sqlx::sqlite::SqliteRow) -> ReviewLog {
    ReviewLog {
        id: row.get("id"),
        card_id: row.get("card_id"),
        deck_id: row.get("deck_id"),
        mode: row.get("mode"),
        rating: row.get("rating"),
        reviewed_at: row.get("reviewed_at"),
    }
}

/// decks 行 → DeckJson（エクスポート用）。
fn deck_json_from_row(row: &sqlx::sqlite::SqliteRow) -> DeckJson {
    let test_modes: String = row.get("test_modes");
    DeckJson {
        schema_version: BACKUP_SCHEMA_VERSION.to_string(),
        deck_id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        language: row
            .try_get::<Option<String>, _>("language")
            .ok()
            .flatten()
            .unwrap_or_else(|| "zh".into()),
        settings: DeckJsonSettings {
            test_modes: serde_json::from_str(&test_modes).unwrap_or_default(),
            daily_new_limit: row.try_get("daily_new_limit").unwrap_or(20),
            daily_review_limit: row.try_get("daily_review_limit").unwrap_or(100),
            fsrs: DeckJsonFsrs {
                target_retention: row.try_get("fsrs_target_retention").unwrap_or(0.90),
                max_interval_days: row.try_get("fsrs_max_interval_days").unwrap_or(365),
            },
        },
    }
}

// ============================================================
// エクスポート
// ============================================================

/// 全データを zip バイト列へ書き出す（コマンドの中核。テストはこちらを叩く）。
async fn build_backup(pool: &Db) -> AppResult<Vec<u8>> {
    let deck_rows = sqlx::query("SELECT * FROM decks ORDER BY id")
        .fetch_all(pool)
        .await?;

    let mut buf = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        let mut deck_ids: Vec<String> = Vec::with_capacity(deck_rows.len());

        let zip_err = |e: zip::result::ZipError| AppError::Io(format!("zip書き込みに失敗: {e}"));

        for drow in &deck_rows {
            let deck_id: String = drow.get("id");
            deck_ids.push(deck_id.clone());

            let dj = deck_json_from_row(drow);

            let card_rows = sqlx::query("SELECT * FROM cards WHERE deck_id = ? ORDER BY id")
                .bind(&deck_id)
                .fetch_all(pool)
                .await?;
            let cards: Vec<Card> = card_rows.iter().map(card_from_row).collect();

            let srs_rows =
                sqlx::query("SELECT * FROM srs_records WHERE deck_id = ? ORDER BY card_id, mode")
                    .bind(&deck_id)
                    .fetch_all(pool)
                    .await?;
            let srs: Vec<SrsRecord> = srs_rows.iter().map(srs_from_row).collect();

            let log_rows =
                sqlx::query("SELECT * FROM review_logs WHERE deck_id = ? ORDER BY reviewed_at, id")
                    .bind(&deck_id)
                    .fetch_all(pool)
                    .await?;
            let logs: Vec<ReviewLog> = log_rows.iter().map(review_log_from_row).collect();

            let base = format!("decks/{deck_id}");
            for (suffix, body) in [
                ("deck.json", serde_json::to_string_pretty(&dj)?),
                ("cards.json", serde_json::to_string(&cards)?),
                ("srs.json", serde_json::to_string(&srs)?),
                ("logs.json", serde_json::to_string(&logs)?),
            ] {
                zw.start_file(format!("{base}/{suffix}"), opts).map_err(zip_err)?;
                zw.write_all(body.as_bytes())?;
            }
        }

        let meta = BackupMeta {
            schema_version: BACKUP_SCHEMA_VERSION.to_string(),
            exported_at: now_rfc3339(),
            deck_ids,
        };
        zw.start_file("backup.json", opts).map_err(zip_err)?;
        zw.write_all(serde_json::to_string_pretty(&meta)?.as_bytes())?;
        zw.finish().map_err(zip_err)?;
    }
    Ok(buf)
}

/// 全データ（全デッキ＋カード＋学習進捗＋履歴）を zip バイト列でエクスポートする。
/// フロントは受け取ったバイト列を `save()` で選んだ保存先へ `writeFile()` する。
#[tauri::command]
#[specta::specta]
pub async fn export_backup(db: tauri::State<'_, Db>) -> AppResult<Vec<u8>> {
    crate::log!(LogLevel::INFO, "Exporting backup");
    let bytes = build_backup(db.inner()).await?;
    crate::log!(LogLevel::INFO, "Backup exported: {} bytes", bytes.len());
    Ok(bytes)
}

// ============================================================
// インポート（マージ復元）
// ============================================================

/// 1 デッキ分の自己完結ユニット（zip 内 `decks/<deck_id>/*.json`）。
#[derive(Default)]
struct BackupUnit {
    deck_json: Option<String>,
    cards: Option<String>,
    srs: Option<String>,
    logs: Option<String>,
}

/// zip を `decks/<deck_id>/` 配下のユニットへ展開する。
/// - zip slip 対策として `enclosed_name()` で安全なエントリのみ扱う。
/// - 返り値は deck_id 昇順（BTreeMap）。
fn extract_backup(bytes: &[u8]) -> AppResult<Vec<(String, BackupUnit)>> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| AppError::Validation(format!("ZIPの読み込みに失敗: {e}")))?;

    let mut units: BTreeMap<String, BackupUnit> = BTreeMap::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::Validation(format!("ZIPエントリの読み込みに失敗: {e}")))?;
        if file.is_dir() {
            continue;
        }
        let path = match file.enclosed_name() {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue, // 危険なパス（../ 等）はスキップ
        };
        let segs: Vec<&str> = path.split('/').collect();
        // 期待形: decks/<deck_id>/<name>.json
        if segs.len() < 3 || segs[0] != "decks" {
            continue;
        }
        let deck_id = segs[1].to_string();
        let fname = segs[segs.len() - 1];
        if deck_id.is_empty() || !fname.to_ascii_lowercase().ends_with(".json") {
            continue;
        }

        let mut text = String::new();
        file.read_to_string(&mut text)
            .map_err(|e| AppError::Validation(format!("ZIP内ファイルの読み込みに失敗 ({fname}): {e}")))?;

        let unit = units.entry(deck_id).or_default();
        match fname {
            "deck.json" => unit.deck_json = Some(text),
            "cards.json" => unit.cards = Some(text),
            "srs.json" => unit.srs = Some(text),
            "logs.json" => unit.logs = Some(text),
            _ => {}
        }
    }

    Ok(units.into_iter().collect())
}

/// decks を upsert（既存は設定を上書き・created_at は維持）。
async fn upsert_deck_tx(
    tx: &mut Transaction<'_, Sqlite>,
    dj: &DeckJson,
    now: &str,
) -> AppResult<()> {
    let test_modes = vec_to_json(&dj.settings.test_modes);
    sqlx::query(
        "INSERT INTO decks \
         (id, name, description, language, test_modes, daily_new_limit, daily_review_limit, \
          fsrs_target_retention, fsrs_max_interval_days, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           name = excluded.name, description = excluded.description, language = excluded.language, \
           test_modes = excluded.test_modes, daily_new_limit = excluded.daily_new_limit, \
           daily_review_limit = excluded.daily_review_limit, \
           fsrs_target_retention = excluded.fsrs_target_retention, \
           fsrs_max_interval_days = excluded.fsrs_max_interval_days, updated_at = excluded.updated_at",
    )
    .bind(&dj.deck_id)
    .bind(&dj.name)
    .bind(&dj.description)
    .bind(&dj.language)
    .bind(&test_modes)
    .bind(dj.settings.daily_new_limit)
    .bind(dj.settings.daily_review_limit)
    .bind(dj.settings.fsrs.target_retention)
    .bind(dj.settings.fsrs.max_interval_days)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// cards を upsert（user_notes・ai_notes・timestamps を含め完全復元）。created を返す。
async fn upsert_card_tx(
    tx: &mut Transaction<'_, Sqlite>,
    deck_id: &str,
    c: &Card,
) -> AppResult<bool> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM cards WHERE id = ? AND deck_id = ?")
            .bind(&c.id)
            .bind(deck_id)
            .fetch_optional(&mut **tx)
            .await?;

    sqlx::query(
        "INSERT INTO cards \
         (id, deck_id, hanzi, pinyin_accepted, meaning, example_sentences, synonyms, antonyms, \
          tags, ai_notes, user_notes, audio_path, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id, deck_id) DO UPDATE SET \
           hanzi = excluded.hanzi, pinyin_accepted = excluded.pinyin_accepted, \
           meaning = excluded.meaning, example_sentences = excluded.example_sentences, \
           synonyms = excluded.synonyms, antonyms = excluded.antonyms, tags = excluded.tags, \
           ai_notes = excluded.ai_notes, user_notes = excluded.user_notes, \
           audio_path = excluded.audio_path, updated_at = excluded.updated_at",
    )
    .bind(&c.id)
    .bind(deck_id)
    .bind(&c.hanzi)
    .bind(vec_to_json(&c.pinyin_accepted))
    .bind(&c.meaning)
    .bind(vec_to_json(&c.example_sentences))
    .bind(vec_to_json(&c.synonyms))
    .bind(vec_to_json(&c.antonyms))
    .bind(vec_to_json(&c.tags))
    .bind(&c.ai_notes)
    .bind(&c.user_notes)
    .bind(&c.audio_path)
    .bind(&c.created_at)
    .bind(&c.updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(existing.is_none())
}

/// srs_records を upsert（同一 card/deck/mode は進捗を上書き）。
async fn upsert_srs_tx(
    tx: &mut Transaction<'_, Sqlite>,
    deck_id: &str,
    r: &SrsRecord,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO srs_records \
         (card_id, deck_id, mode, due_date, stability, difficulty, state, reps, lapses, \
          last_review, scheduled_days, elapsed_days) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(card_id, deck_id, mode) DO UPDATE SET \
           due_date = excluded.due_date, stability = excluded.stability, \
           difficulty = excluded.difficulty, state = excluded.state, reps = excluded.reps, \
           lapses = excluded.lapses, last_review = excluded.last_review, \
           scheduled_days = excluded.scheduled_days, elapsed_days = excluded.elapsed_days",
    )
    .bind(&r.card_id)
    .bind(deck_id)
    .bind(&r.mode)
    .bind(&r.due_date)
    .bind(r.stability)
    .bind(r.difficulty)
    .bind(&r.state)
    .bind(r.reps)
    .bind(r.lapses)
    .bind(&r.last_review)
    .bind(r.scheduled_days)
    .bind(r.elapsed_days)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// review_logs を追加（id は UUID。重複は無視 = 履歴は二重取り込みしない）。挿入件数を返す。
async fn insert_log_ignore_tx(
    tx: &mut Transaction<'_, Sqlite>,
    deck_id: &str,
    l: &ReviewLog,
) -> AppResult<u32> {
    let res = sqlx::query(
        "INSERT OR IGNORE INTO review_logs (id, card_id, deck_id, mode, rating, reviewed_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&l.id)
    .bind(&l.card_id)
    .bind(deck_id)
    .bind(&l.mode)
    .bind(&l.rating)
    .bind(&l.reviewed_at)
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() as u32)
}

/// zip バイト列をマージ復元する（コマンドの中核。テストはこちらを叩く）。
/// `only` が Some の場合、そのデッキIDのユニットだけを復元する（選択復元）。
async fn restore_backup(
    pool: &Db,
    data: &[u8],
    only: Option<Vec<String>>,
) -> AppResult<BackupImportResult> {
    let units = extract_backup(data)?;
    if units.is_empty() {
        return Err(AppError::Validation(
            "バックアップ内に decks/ が見つかりません".into(),
        ));
    }
    let filter: Option<HashSet<String>> = only.map(|v| v.into_iter().collect());
    let now = now_rfc3339();
    let mut result = BackupImportResult::default();

    // 全体を 1 トランザクションで（all-or-nothing）。外部キー順に投入する。
    let mut tx = pool.begin().await?;
    for (deck_id, unit) in &units {
        if let Some(f) = &filter {
            if !f.contains(deck_id) {
                continue;
            }
        }

        let deck_text = unit.deck_json.as_ref().ok_or_else(|| {
            AppError::Validation(format!("deck.json が無いユニット: {deck_id}"))
        })?;
        let dj: DeckJson = serde_json::from_str(deck_text)
            .map_err(|e| AppError::Validation(format!("deck.json の解析に失敗 ({deck_id}): {e}")))?;
        if dj.schema_version != BACKUP_SCHEMA_VERSION {
            return Err(AppError::Validation(format!(
                "未対応のバックアップ版です（{}）",
                dj.schema_version
            )));
        }
        upsert_deck_tx(&mut tx, &dj, &now).await?;
        result.decks += 1;

        if let Some(t) = &unit.cards {
            let cards: Vec<Card> = serde_json::from_str(t).map_err(|e| {
                AppError::Validation(format!("cards.json の解析に失敗 ({deck_id}): {e}"))
            })?;
            for c in &cards {
                if upsert_card_tx(&mut tx, deck_id, c).await? {
                    result.cards_created += 1;
                } else {
                    result.cards_updated += 1;
                }
            }
        }

        if let Some(t) = &unit.srs {
            let recs: Vec<SrsRecord> = serde_json::from_str(t).map_err(|e| {
                AppError::Validation(format!("srs.json の解析に失敗 ({deck_id}): {e}"))
            })?;
            for r in &recs {
                upsert_srs_tx(&mut tx, deck_id, r).await?;
                result.srs_imported += 1;
            }
        }

        if let Some(t) = &unit.logs {
            let logs: Vec<ReviewLog> = serde_json::from_str(t).map_err(|e| {
                AppError::Validation(format!("logs.json の解析に失敗 ({deck_id}): {e}"))
            })?;
            for l in &logs {
                result.logs_imported += insert_log_ignore_tx(&mut tx, deck_id, l).await?;
            }
        }
    }
    tx.commit().await?;
    Ok(result)
}

/// バックアップ zip バイト列をマージ復元する（content:// 対応のためバイト列受け）。
/// `deck_ids` を指定すると、そのデッキだけを選択復元する（null = 全件）。
#[tauri::command]
#[specta::specta]
pub async fn import_backup(
    db: tauri::State<'_, Db>,
    data: Vec<u8>,
    deck_ids: Option<Vec<String>>,
) -> AppResult<BackupImportResult> {
    crate::log!(
        LogLevel::INFO,
        "Importing backup ({} bytes, filter={:?})",
        data.len(),
        deck_ids
    );
    let res = restore_backup(db.inner(), &data, deck_ids).await?;
    crate::log!(
        LogLevel::INFO,
        "Backup import done: decks={}, cards={}/{}, srs={}, logs={}",
        res.decks,
        res.cards_created,
        res.cards_updated,
        res.srs_imported,
        res.logs_imported
    );
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 デッキ・1 カード・1 srs・1 log を投入したプールを作る。
    async fn seed_pool(tag: &str) -> Db {
        let db_path =
            std::env::temp_dir().join(format!("ankiros_bak_{tag}_{}.db", uuid::Uuid::new_v4()));
        let pool = crate::db::init_pool(&db_path).await.unwrap();
        let now = "2026-06-26T00:00:00+00:00";

        sqlx::query(
            "INSERT INTO decks (id, name, description, language, test_modes, daily_new_limit, \
             daily_review_limit, fsrs_target_retention, fsrs_max_interval_days, created_at, updated_at) \
             VALUES ('d1','My Deck','desc','zh','[\"recognition\"]',20,100,0.9,365,?,?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO cards (id, deck_id, hanzi, pinyin_accepted, meaning, example_sentences, \
             synonyms, antonyms, tags, ai_notes, user_notes, audio_path, created_at, updated_at) \
             VALUES ('c1','d1','好','[\"hao3\"]','good','[]','[]','[]','[]','ai','MY NOTE',NULL,?,?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO srs_records (card_id, deck_id, mode, due_date, stability, difficulty, \
             state, reps, lapses, last_review, scheduled_days, elapsed_days) \
             VALUES ('c1','d1','recognition','2026-07-01T00:00:00+00:00',12.5,5.0,'review',3,1,?,7,2)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO review_logs (id, card_id, deck_id, mode, rating, reviewed_at) \
             VALUES ('log1','c1','d1','recognition','good',?)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn export_then_import_roundtrip_preserves_progress() {
        let src = seed_pool("src").await;
        let bytes = build_backup(&src).await.unwrap();
        assert!(!bytes.is_empty());

        // 別の空 DB へ復元
        let dst = seed_empty().await;
        let res = restore_backup(&dst, &bytes, None).await.unwrap();
        assert_eq!(res.decks, 1);
        assert_eq!(res.cards_created, 1);
        assert_eq!(res.srs_imported, 1);
        assert_eq!(res.logs_imported, 1);

        // 学習進捗（srs）が忠実に復元されているか
        let row = sqlx::query("SELECT stability, reps, state FROM srs_records WHERE card_id='c1'")
            .fetch_one(&dst)
            .await
            .unwrap();
        let stability: f64 = row.get("stability");
        let reps: i64 = row.get("reps");
        let state: String = row.get("state");
        assert_eq!(reps, 3);
        assert_eq!(state, "review");
        assert!((stability - 12.5).abs() < 1e-9);

        // user_notes も復元されているか
        let note: String = sqlx::query("SELECT user_notes FROM cards WHERE id='c1'")
            .fetch_one(&dst)
            .await
            .unwrap()
            .get("user_notes");
        assert_eq!(note, "MY NOTE");
    }

    #[tokio::test]
    async fn reimport_is_idempotent_for_logs() {
        let src = seed_pool("idem").await;
        let bytes = build_backup(&src).await.unwrap();
        let dst = seed_empty().await;

        let r1 = restore_backup(&dst, &bytes, None).await.unwrap();
        assert_eq!(r1.logs_imported, 1);
        // 2 回目: カードは updated、ログは重複なので 0 件追加
        let r2 = restore_backup(&dst, &bytes, None).await.unwrap();
        assert_eq!(r2.cards_created, 0);
        assert_eq!(r2.cards_updated, 1);
        assert_eq!(r2.logs_imported, 0);

        let log_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM review_logs")
            .fetch_one(&dst)
            .await
            .unwrap()
            .get("c");
        assert_eq!(log_count, 1);
    }

    #[tokio::test]
    async fn selective_restore_filters_by_deck_id() {
        let src = seed_pool("sel").await;
        let bytes = build_backup(&src).await.unwrap();
        let dst = seed_empty().await;

        // 存在しないデッキIDのみ指定 → 何も復元されない
        let res = restore_backup(&dst, &bytes, Some(vec!["other".into()]))
            .await
            .unwrap();
        assert_eq!(res.decks, 0);
        let deck_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM decks")
            .fetch_one(&dst)
            .await
            .unwrap()
            .get("c");
        assert_eq!(deck_count, 0);
    }

    async fn seed_empty() -> Db {
        let db_path =
            std::env::temp_dir().join(format!("ankiros_bak_empty_{}.db", uuid::Uuid::new_v4()));
        crate::db::init_pool(&db_path).await.unwrap()
    }
}
