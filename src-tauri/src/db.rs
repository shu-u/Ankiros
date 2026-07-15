use crate::error::AppResult;
use crate::models::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub type Db = SqlitePool;

/// リスニング出題が有効か（app_state.listening_enabled, 既定 true）。
/// 無音環境トグル用。false のとき listening モードをセッションキュー・
/// 進捗の分母から動的に除外する（srs_records 自体は保持したまま）。
pub async fn listening_enabled(pool: &Db) -> bool {
    sqlx::query("SELECT value FROM app_state WHERE key = 'listening_enabled'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| r.get::<String, _>("value") != "false")
        .unwrap_or(true)
}

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
// 今日導入した新規ユニット数（日次の新規上限を効かせるため）
// ------------------------------------------------------------

/// デッキ内で「今日(JST) 初めて学習した (card_id, mode) ユニット数」を返す。
/// review_logs における各 (card_id, mode) の最初の reviewed_at が今日のものを数える。
/// daily_new_limit から差し引くことで、1日あたりの新規導入を正しく上限化する。
pub async fn new_introduced_today(
    pool: &Db,
    deck_id: &str,
    today: chrono::NaiveDate,
) -> AppResult<i64> {
    use std::collections::HashMap;
    let rows = sqlx::query("SELECT card_id, mode, reviewed_at FROM review_logs WHERE deck_id = ?")
        .bind(deck_id)
        .fetch_all(pool)
        .await?;
    // (card_id, mode) ごとの最初の学習日(JST)を求める
    let mut earliest: HashMap<(String, String), chrono::NaiveDate> = HashMap::new();
    for r in &rows {
        let cid: String = r.get("card_id");
        let mode: String = r.get("mode");
        let at: String = r.get("reviewed_at");
        if let Some(d) = crate::util::to_jst_date(&at) {
            earliest
                .entry((cid, mode))
                .and_modify(|cur| {
                    if d < *cur {
                        *cur = d;
                    }
                })
                .or_insert(d);
        }
    }
    Ok(earliest.values().filter(|d| **d == today).count() as i64)
}

// ------------------------------------------------------------
// 出題対象モード（統計とセッションの集計対象を一致させる）
// ------------------------------------------------------------

/// デッキの「出題対象モード」を返す。test_modes(JSON) から、リスニング無効時の
/// 'listening' を除外したもの。セッションキューが実際に出題するモード集合と同じで、
/// 統計（ホーム/デッキ詳細/先読み）の集計対象をこれに揃えることで件数の不一致を防ぐ。
pub fn effective_modes(test_modes_json: &str, listening_on: bool) -> Vec<String> {
    let mut modes: Vec<String> = serde_json::from_str(test_modes_json).unwrap_or_default();
    if !listening_on {
        modes.retain(|m| m != "listening");
    }
    modes
}

/// デッキ内で「state 条件・期日到来(due < cutoff)・出題対象モード」に合致する srs_records 件数。
/// `modes` が空（出題対象モード無し）なら 0。`state_clause` はコード内の固定リテラルのみ渡すこと
/// （SQL に直接埋め込むため、外部入力を渡してはならない）。
pub async fn count_due_by_modes(
    pool: &Db,
    deck_id: &str,
    state_clause: &str,
    modes: &[String],
    due_cutoff: &str,
) -> AppResult<i64> {
    if modes.is_empty() {
        return Ok(0);
    }
    let placeholders = std::iter::repeat("?")
        .take(modes.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT COUNT(*) AS c FROM srs_records \
         WHERE deck_id = ? AND {state_clause} AND due_date < ? AND mode IN ({placeholders})"
    );
    let mut q = sqlx::query(&sql).bind(deck_id).bind(due_cutoff);
    for m in modes {
        q = q.bind(m);
    }
    Ok(q.fetch_one(pool).await?.get("c"))
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
