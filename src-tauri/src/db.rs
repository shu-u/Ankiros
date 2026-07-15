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

/// 今日の「復習負荷」= 出題対象モードの、当日新規でない復習系ユニットのうち今日スコープに入る数。
///
/// スコープ = 「今日すでに実施済み」∪「未消化で期日到来(state != 'new')」を (card_id, mode) で
/// 重複排除し、`review_limit` で頭打ちしたもの。
///
/// 当日導入した新規ユニット（最初の学習日 == today）は新規側で計上済みのため除外する。
/// 実施すると due から外れて「実施済み」に移るだけなので、和集合の要素数は1日を通して不変。
/// これにより「学習量の目安」による新規の絞り込みが、セッションを何度開き直しても、
/// 復習を消化した後でもブレない（＝途中で新規が勝手に増減しない）。
pub async fn review_load_today(
    pool: &Db,
    deck_id: &str,
    modes: &[String],
    due_cutoff: &str,
    today: chrono::NaiveDate,
    review_limit: i64,
) -> AppResult<i64> {
    use std::collections::{HashMap, HashSet};
    if modes.is_empty() || review_limit <= 0 {
        return Ok(0);
    }
    let mode_set: HashSet<&str> = modes.iter().map(|s| s.as_str()).collect();

    // review_logs から (card,mode) ごとの「最初の学習日」と「今日学習したか」を求める。
    let log_rows = sqlx::query("SELECT card_id, mode, reviewed_at FROM review_logs WHERE deck_id = ?")
        .bind(deck_id)
        .fetch_all(pool)
        .await?;
    let mut earliest: HashMap<(String, String), chrono::NaiveDate> = HashMap::new();
    let mut logged_today: HashSet<(String, String)> = HashSet::new();
    for r in &log_rows {
        let cid: String = r.get("card_id");
        let mode: String = r.get("mode");
        let at: String = r.get("reviewed_at");
        if let Some(d) = crate::util::to_jst_date(&at) {
            if d == today {
                logged_today.insert((cid.clone(), mode.clone()));
            }
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
    // 当日新規判定: 最初の学習日が today のユニット。
    let is_new_today =
        |cid: &str, mode: &str| earliest.get(&(cid.to_string(), mode.to_string())) == Some(&today);

    let mut scope: HashSet<(String, String)> = HashSet::new();

    // A: 今日実施済み ∩ 出題対象モード ∩ 当日新規でない
    for (cid, mode) in &logged_today {
        if mode_set.contains(mode.as_str()) && !is_new_today(cid, mode) {
            scope.insert((cid.clone(), mode.clone()));
        }
    }

    // B: 未消化で期日到来(state != 'new') ∩ 出題対象モード ∩ 当日新規でない
    let placeholders = std::iter::repeat("?")
        .take(modes.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT card_id, mode FROM srs_records \
         WHERE deck_id = ? AND state != 'new' AND due_date < ? AND mode IN ({placeholders})"
    );
    let mut q = sqlx::query(&sql).bind(deck_id).bind(due_cutoff);
    for m in modes {
        q = q.bind(m);
    }
    for r in &q.fetch_all(pool).await? {
        let cid: String = r.get("card_id");
        let mode: String = r.get("mode");
        if !is_new_today(&cid, &mode) {
            scope.insert((cid, mode));
        }
    }

    Ok((scope.len() as i64).min(review_limit))
}

/// 実効新規上限の内訳（ログ・UI 用に導入済み数と復習負荷も返す）。
pub struct NewLimitBreakdown {
    /// 今日出題してよい新規ユニット数（0 以上）。
    pub effective: i64,
    /// 今日すでに導入した新規ユニット数。
    pub introduced_today: i64,
    /// 復習負荷（study_target が有効なときのみ Some）。
    pub review_load: Option<i64>,
}

/// 今日の実効新規上限を算出する（学習量の目安によるスロットル込み）。
///
/// base = (new_limit − 今日導入済み).max(0) … 従来の日次新規上限。
/// study_target が Some のときは新規のみを絞る:
///   effective = base.min((study_target − 復習負荷).max(0))
/// 復習は絞らない（常に review_limit まで出題される）。
///
/// すべて `.max(0)` でクランプするため、new/review/study のいずれを1日の途中で変更しても、
/// また負値・過大値・0 といった異常値が入っても、負の上限（＝truncate 破綻）にはならない。
pub async fn effective_new_limit(
    pool: &Db,
    deck_id: &str,
    modes: &[String],
    due_cutoff: &str,
    today: chrono::NaiveDate,
    new_limit: i64,
    review_limit: i64,
    study_target: Option<i64>,
) -> AppResult<NewLimitBreakdown> {
    let introduced_today = new_introduced_today(pool, deck_id, today).await?;
    let base = (new_limit - introduced_today).max(0);
    match study_target {
        Some(target) => {
            let load =
                review_load_today(pool, deck_id, modes, due_cutoff, today, review_limit).await?;
            Ok(NewLimitBreakdown {
                effective: base.min((target - load).max(0)),
                introduced_today,
                review_load: Some(load),
            })
        }
        None => Ok(NewLimitBreakdown {
            effective: base,
            introduced_today,
            review_load: None,
        }),
    }
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

#[cfg(test)]
mod tests {
    //! 学習量の目安（新規スロットル）の中核ロジックの検証。
    //! 重点: 1日の途中で復習を消化しても復習負荷が不変であること（新規が勝手に増減しない）、
    //! 当日新規の除外、review_limit による頭打ち、異常値のクランプ。
    use super::*;
    use chrono::NaiveDate;

    const TODAY: &str = "2026-07-15";
    // JST 正午 → 論理日は当日。earliest がこれだけのユニットは「当日新規」とみなされる。
    const LOG_TODAY: &str = "2026-07-15T03:00:00+00:00";
    // 前日の学習ログ（earliest < today → 既存＝当日新規でない）。
    const LOG_YESTERDAY: &str = "2026-07-14T03:00:00+00:00";
    const DUE_PAST: &str = "2026-07-10T00:00:00+00:00"; // 期日到来（cutoff より前）
    const DUE_FUTURE: &str = "2026-08-01T00:00:00+00:00"; // 未消化でない（未来）

    fn today() -> NaiveDate {
        NaiveDate::parse_from_str(TODAY, "%Y-%m-%d").unwrap()
    }
    fn cutoff() -> String {
        let now = chrono::DateTime::parse_from_rfc3339(LOG_TODAY)
            .unwrap()
            .with_timezone(&chrono::Utc);
        crate::util::logical_day_end(now).to_rfc3339()
    }
    fn modes() -> Vec<String> {
        vec!["recognition".to_string()]
    }

    async fn pool() -> Db {
        let p = std::env::temp_dir().join(format!("ankiros_dbtest_{}.db", uuid::Uuid::new_v4()));
        init_pool(&p).await.unwrap()
    }

    async fn seed_deck(pool: &Db, study_target: Option<i64>, new_limit: i64, review_limit: i64) {
        sqlx::query(
            "INSERT INTO decks (id,name,test_modes,daily_new_limit,daily_review_limit,\
             daily_study_target,created_at,updated_at) \
             VALUES ('d','D','[\"recognition\"]',?,?,?,'x','x')",
        )
        .bind(new_limit)
        .bind(review_limit)
        .bind(study_target)
        .execute(pool)
        .await
        .unwrap();
    }

    /// カード＋srs_record（1モード）＋任意の学習ログを投入する。
    async fn add_unit(pool: &Db, id: &str, due: &str, state: &str, logs: &[&str]) {
        sqlx::query(
            "INSERT INTO cards (id,deck_id,hanzi,pinyin_accepted,meaning,created_at,updated_at) \
             VALUES (?,'d','h','[]','m','x','x')",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO srs_records (card_id,deck_id,mode,due_date,state) \
             VALUES (?,'d','recognition',?,?)",
        )
        .bind(id)
        .bind(due)
        .bind(state)
        .execute(pool)
        .await
        .unwrap();
        for at in logs {
            sqlx::query(
                "INSERT INTO review_logs (id,card_id,deck_id,mode,rating,reviewed_at) \
                 VALUES (?,?,'d','recognition','good',?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(id)
            .bind(*at)
            .execute(pool)
            .await
            .unwrap();
        }
    }

    async fn calc(pool: &Db, new_limit: i64, review_limit: i64, target: Option<i64>) -> NewLimitBreakdown {
        effective_new_limit(pool, "d", &modes(), &cutoff(), today(), new_limit, review_limit, target)
            .await
            .unwrap()
    }

    // 復習90件溜まっている日は新規10（例: new=20, review=100, study=100）。
    #[tokio::test]
    async fn throttle_reduces_new_by_review_load() {
        let pool = pool().await;
        seed_deck(&pool, Some(100), 20, 100).await;
        for i in 0..90 {
            add_unit(&pool, &format!("c{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        let load = review_load_today(&pool, "d", &modes(), &cutoff(), today(), 100).await.unwrap();
        assert_eq!(load, 90);
        let c = calc(&pool, 20, 100, Some(100)).await;
        assert_eq!(c.effective, 10);
        assert_eq!(c.introduced_today, 0);
    }

    // 途中変更耐性の要: 50件消化(未来送り＋当日ログ)＋40件未消化 でも負荷は 90 のまま → 新規は10で不変。
    #[tokio::test]
    async fn review_load_is_stable_after_clearing_reviews() {
        let pool = pool().await;
        seed_deck(&pool, Some(100), 20, 100).await;
        for i in 0..50 {
            add_unit(&pool, &format!("done{i}"), DUE_FUTURE, "review", &[LOG_YESTERDAY, LOG_TODAY]).await;
        }
        for i in 0..40 {
            add_unit(&pool, &format!("due{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        let load = review_load_today(&pool, "d", &modes(), &cutoff(), today(), 100).await.unwrap();
        assert_eq!(load, 90);
        assert_eq!(calc(&pool, 20, 100, Some(100)).await.effective, 10);
    }

    // 無効(None)なら従来どおり新規は日次上限まで（負荷計算もスキップ）。
    #[tokio::test]
    async fn disabled_target_keeps_full_new_budget() {
        let pool = pool().await;
        seed_deck(&pool, None, 20, 100).await;
        for i in 0..90 {
            add_unit(&pool, &format!("c{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        let c = calc(&pool, 20, 100, None).await;
        assert_eq!(c.effective, 20);
        assert_eq!(c.review_load, None);
    }

    // 復習負荷は review_limit で頭打ち（review_limit を途中で下げても破綻せず、負荷は上限まで）。
    #[tokio::test]
    async fn review_limit_caps_the_load() {
        let pool = pool().await;
        seed_deck(&pool, Some(100), 20, 50).await;
        for i in 0..90 {
            add_unit(&pool, &format!("c{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        let load = review_load_today(&pool, "d", &modes(), &cutoff(), today(), 50).await.unwrap();
        assert_eq!(load, 50);
        // 100 - 50 = 50 >= 20 → 新規はフル
        assert_eq!(calc(&pool, 20, 50, Some(100)).await.effective, 20);
    }

    // 当日導入の新規は復習負荷から除外され、かつ introduced_today に計上される。
    #[tokio::test]
    async fn today_new_cards_excluded_from_load_and_counted_as_introduced() {
        let pool = pool().await;
        seed_deck(&pool, Some(100), 20, 100).await;
        for i in 0..90 {
            add_unit(&pool, &format!("old{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        for i in 0..5 {
            add_unit(&pool, &format!("new{i}"), DUE_PAST, "learning", &[LOG_TODAY]).await;
        }
        let load = review_load_today(&pool, "d", &modes(), &cutoff(), today(), 100).await.unwrap();
        assert_eq!(load, 90); // 当日新規5件は負荷に含まない
        let c = calc(&pool, 20, 100, Some(100)).await;
        assert_eq!(c.introduced_today, 5);
        // base = 20 - 5 = 15、(100 - 90) = 10 → min = 10
        assert_eq!(c.effective, 10);
    }

    // 異常値(0/負)でも新規は 0 にクランプされ、負値（truncate 破綻）にならない。
    #[tokio::test]
    async fn zero_or_negative_target_clamps_new_to_zero() {
        let pool = pool().await;
        seed_deck(&pool, Some(0), 20, 100).await;
        for i in 0..3 {
            add_unit(&pool, &format!("c{i}"), DUE_PAST, "review", &[LOG_YESTERDAY]).await;
        }
        assert_eq!(calc(&pool, 20, 100, Some(0)).await.effective, 0);
        assert_eq!(calc(&pool, 20, 100, Some(-5)).await.effective, 0);
    }
}
