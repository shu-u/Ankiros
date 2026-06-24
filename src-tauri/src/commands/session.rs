use crate::db::{card_from_row, Db};
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::srs::compute_review;
use crate::util::now_rfc3339;
use rand::seq::SliceRandom;
use sqlx::Row;
use uuid::Uuid;

/// セッションキューを構築する (spec §6.1)。
/// daily_new_limit / daily_review_limit はモード横断の合計として適用する。
#[tauri::command]
#[specta::specta]
pub async fn get_session_queue(db: tauri::State<'_, Db>, deck_id: String) -> AppResult<Vec<SessionCard>> {
    crate::log!(LogLevel::DEBUG, "get_session_queue: deck={}", deck_id);
    let pool = db.inner();
    let now = now_rfc3339();

    let deck_row = sqlx::query("SELECT test_modes, daily_new_limit, daily_review_limit FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let modes_json: String = deck_row.get("test_modes");
    let test_modes: Vec<String> = serde_json::from_str(&modes_json).unwrap_or_default();
    let new_limit: i64 = deck_row.try_get("daily_new_limit").unwrap_or(20);
    let review_limit: i64 = deck_row.try_get("daily_review_limit").unwrap_or(100);

    // 最後に使ったデッキを記録 (spec §3.3 app_state)。
    // ※ シャッフル用 rng (!Send) を await をまたいで保持しないよう、先に実行しておく。
    sqlx::query(
        "INSERT INTO app_state (key, value) VALUES ('last_used_deck_id', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&deck_id)
    .execute(pool)
    .await?;

    // 3. 新規スタディユニット（モード横断）
    let mut new_pool: Vec<SessionCard> = Vec::new();
    for mode in &test_modes {
        let rows = sqlx::query(
            "SELECT c.* FROM cards c \
             LEFT JOIN srs_records sr \
               ON c.id = sr.card_id AND c.deck_id = sr.deck_id AND sr.mode = ? \
             WHERE c.deck_id = ? AND sr.card_id IS NULL",
        )
        .bind(mode)
        .bind(&deck_id)
        .fetch_all(pool)
        .await?;
        for row in &rows {
            new_pool.push(SessionCard {
                card: card_from_row(row),
                mode: mode.clone(),
                srs_state: "new".to_string(),
            });
        }
    }

    // 4. 復習スタディユニット（モード横断）
    let mut review_pool: Vec<SessionCard> = Vec::new();
    for mode in &test_modes {
        let rows = sqlx::query(
            "SELECT c.*, sr.state AS srs_state FROM cards c \
             JOIN srs_records sr \
               ON c.id = sr.card_id AND c.deck_id = sr.deck_id AND sr.mode = ? \
             WHERE c.deck_id = ? AND sr.state != 'new' AND sr.due_date <= ?",
        )
        .bind(mode)
        .bind(&deck_id)
        .bind(&now)
        .fetch_all(pool)
        .await?;
        for row in &rows {
            let srs_state: String = row.get("srs_state");
            review_pool.push(SessionCard {
                card: card_from_row(row),
                mode: mode.clone(),
                srs_state,
            });
        }
    }

    crate::log!(
        LogLevel::DEBUG,
        "session candidates: new={}, review={} (limits new={}, review={})",
        new_pool.len(),
        review_pool.len(),
        new_limit,
        review_limit
    );

    // 5-8. シャッフル → 上限適用 → 結合 → 再シャッフル
    // rng は !Send なため、この同期ブロック内で完結させ await をまたがない。
    let queue = {
        let mut rng = rand::thread_rng();
        new_pool.shuffle(&mut rng);
        review_pool.shuffle(&mut rng);
        new_pool.truncate(new_limit.max(0) as usize);
        review_pool.truncate(review_limit.max(0) as usize);

        let mut queue = new_pool;
        queue.extend(review_pool);
        queue.shuffle(&mut rng);
        queue
    };

    crate::log!(
        LogLevel::INFO,
        "Session queue built: deck={}, {} cards",
        deck_id,
        queue.len()
    );
    Ok(queue)
}

/// 回答前に各評価ボタンの次回間隔ラベルを計算する（DBには書き込まない）。
/// spec §6.4 の「今日中 / N日後」表示用。
#[tauri::command]
#[specta::specta]
pub async fn preview_review(
    db: tauri::State<'_, Db>,
    card_id: String,
    deck_id: String,
    mode: String,
) -> AppResult<IntervalPreview> {
    crate::log!(LogLevel::VERBOSE, "preview_review: {}/{} mode={}", deck_id, card_id, mode);
    let pool = db.inner();
    let now_dt = chrono::Utc::now();

    let deck_row = sqlx::query("SELECT fsrs_target_retention, fsrs_max_interval_days FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let target_retention: f64 = deck_row.try_get("fsrs_target_retention").unwrap_or(0.90);
    let max_interval: i64 = deck_row.try_get("fsrs_max_interval_days").unwrap_or(365);

    let existing_row = sqlx::query(
        "SELECT * FROM srs_records WHERE card_id = ? AND deck_id = ? AND mode = ?",
    )
    .bind(&card_id)
    .bind(&deck_id)
    .bind(&mode)
    .fetch_optional(pool)
    .await?;
    let existing = existing_row.as_ref().map(crate::db::srs_from_row);

    // rating は任意（プレビュー部分は repeat により全評価分が算出される）
    let comp = compute_review(
        existing.as_ref(),
        &card_id,
        &deck_id,
        &mode,
        "good",
        target_retention,
        max_interval,
        now_dt,
    )?;
    Ok(comp.preview)
}

/// 回答を採点し FSRS でスケジューリングする (spec §12 submit_review)
#[tauri::command]
#[specta::specta]
pub async fn submit_review(
    db: tauri::State<'_, Db>,
    card_id: String,
    deck_id: String,
    mode: String,
    rating: String,
) -> AppResult<SubmitReviewResult> {
    let pool = db.inner();
    let now = now_rfc3339();
    let now_dt = chrono::DateTime::parse_from_rfc3339(&now)
        .unwrap()
        .with_timezone(&chrono::Utc);

    // デッキの FSRS パラメータ
    let deck_row = sqlx::query("SELECT fsrs_target_retention, fsrs_max_interval_days FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let target_retention: f64 = deck_row.try_get("fsrs_target_retention").unwrap_or(0.90);
    let max_interval: i64 = deck_row.try_get("fsrs_max_interval_days").unwrap_or(365);

    // 既存 srs_record（無ければ None → fsrs::Card::default()）
    let existing_row = sqlx::query(
        "SELECT * FROM srs_records WHERE card_id = ? AND deck_id = ? AND mode = ?",
    )
    .bind(&card_id)
    .bind(&deck_id)
    .bind(&mode)
    .fetch_optional(pool)
    .await?;
    let existing = existing_row.as_ref().map(crate::db::srs_from_row);

    let comp = compute_review(
        existing.as_ref(),
        &card_id,
        &deck_id,
        &mode,
        &rating,
        target_retention,
        max_interval,
        now_dt,
    )?;
    let u = &comp.updated;

    // UPSERT srs_records
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
    .bind(&u.card_id)
    .bind(&u.deck_id)
    .bind(&u.mode)
    .bind(&u.due_date)
    .bind(u.stability)
    .bind(u.difficulty)
    .bind(&u.state)
    .bind(u.reps)
    .bind(u.lapses)
    .bind(&u.last_review)
    .bind(u.scheduled_days)
    .bind(u.elapsed_days)
    .execute(pool)
    .await?;

    // review_logs に記録
    sqlx::query(
        "INSERT INTO review_logs (id, card_id, deck_id, mode, rating, reviewed_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&card_id)
    .bind(&deck_id)
    .bind(&mode)
    .bind(&rating)
    .bind(&now)
    .execute(pool)
    .await?;

    crate::log!(
        LogLevel::DEBUG,
        "submit_review: {}/{} mode={} rating={} -> state={}, requeue={}",
        deck_id,
        card_id,
        mode,
        rating,
        comp.updated.state,
        comp.should_requeue
    );

    Ok(SubmitReviewResult {
        updated_srs: comp.updated,
        should_requeue: comp.should_requeue,
        interval_preview: comp.preview,
    })
}
