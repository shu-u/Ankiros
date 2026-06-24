use crate::error::AppResult;
use crate::models::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub type Db = SqlitePool;

/// SQLite プールを初期化する。
/// - create_if_missing: 初回起動時に空DBを作成 (spec §3.1)
/// - foreign_keys(true): 全プール接続で PRAGMA foreign_keys = ON (spec §3.3/§13)
/// - WAL: 並行性向上
/// 接続後、sqlx::migrate! で未適用マイグレーションを自動適用する。
pub async fn init_pool(db_path: &Path) -> AppResult<Db> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.to_string_lossy()))
        .map_err(|e| crate::error::AppError::Database(e.to_string()))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

// ------------------------------------------------------------
// JSON カラムのシリアライズ／デシリアライズ
// ------------------------------------------------------------

pub fn vec_to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
}

pub fn json_to_vec<T: serde::de::DeserializeOwned + Default>(s: Option<String>) -> T {
    match s {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(&text).unwrap_or_default(),
        _ => T::default(),
    }
}

// ------------------------------------------------------------
// 行 → モデル 変換
// ------------------------------------------------------------

pub fn card_from_row(row: &sqlx::sqlite::SqliteRow) -> Card {
    Card {
        id: row.get("id"),
        deck_id: row.get("deck_id"),
        hanzi: row.get("hanzi"),
        pinyin_accepted: json_to_vec(row.get("pinyin_accepted")),
        meaning: row.get("meaning"),
        example_sentences: json_to_vec(row.get("example_sentences")),
        synonyms: json_to_vec(row.get("synonyms")),
        antonyms: json_to_vec(row.get("antonyms")),
        tags: json_to_vec(row.get("tags")),
        ai_notes: row.get("ai_notes"),
        user_notes: row
            .try_get::<Option<String>, _>("user_notes")
            .ok()
            .flatten()
            .unwrap_or_default(),
        audio_path: row.get("audio_path"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub fn srs_from_row(row: &sqlx::sqlite::SqliteRow) -> SrsRecord {
    SrsRecord {
        card_id: row.get("card_id"),
        deck_id: row.get("deck_id"),
        mode: row.get("mode"),
        due_date: row.get("due_date"),
        stability: row.get("stability"),
        difficulty: row.get("difficulty"),
        state: row
            .try_get::<Option<String>, _>("state")
            .ok()
            .flatten()
            .unwrap_or_else(|| "new".to_string()),
        reps: row.try_get("reps").unwrap_or(0),
        lapses: row.try_get("lapses").unwrap_or(0),
        last_review: row.get("last_review"),
        scheduled_days: row.try_get("scheduled_days").unwrap_or(0),
        elapsed_days: row.try_get("elapsed_days").unwrap_or(0),
    }
}
