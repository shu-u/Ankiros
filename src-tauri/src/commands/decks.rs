use crate::db::{vec_to_json, Db};
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::util::{now_rfc3339, validate_id};
use sqlx::Row;

/// 学習量の目安の入力検証。壊れ値（負値）のみ弾く。
/// 上限側は new/review と独立に変更できるよう相互チェックはしない
/// （過大値は「実質無効」として `effective_new_limit` 側で安全に吸収される）。
fn validate_study_target(study_target: Option<i64>) -> AppResult<()> {
    if let Some(t) = study_target {
        if t < 0 {
            return Err(AppError::Validation(
                "1日の学習量の目安は0以上で入力してください".into(),
            ));
        }
    }
    Ok(())
}

/// 1デッキ分の派生カウントを算出。
/// 戻り値: (カード総数, 新規予定, 復習予定, 学習中, 学習量の目安で新規が絞られたか)。
/// 新規予定は「今日導入済み」を差し引いた実効上限＋学習量の目安によるスロットルを反映する。
/// 復習/学習中はセッションが出題する「出題対象モード（`modes`）」のみを対象にする。
async fn deck_counts(
    pool: &Db,
    deck_id: &str,
    modes: &[String],
    due_cutoff: &str,
) -> AppResult<(i64, i64, i64, i64, bool)> {
    let card_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM cards WHERE deck_id = ?")
        .bind(deck_id)
        .fetch_one(pool)
        .await?
        .get("c");

    // デッキの上限一式（新規・復習・学習量の目安）
    let limits = sqlx::query(
        "SELECT daily_new_limit, daily_review_limit, daily_study_target FROM decks WHERE id = ?",
    )
    .bind(deck_id)
    .fetch_optional(pool)
    .await?;
    let (new_limit, review_limit, study_target) = match &limits {
        Some(r) => (
            r.try_get("daily_new_limit").unwrap_or(20),
            r.try_get("daily_review_limit").unwrap_or(100),
            r.try_get::<Option<i64>, _>("daily_study_target").ok().flatten(),
        ),
        None => (20, 100, None),
    };

    // 復習（state='review' かつ期日到来 = 今日中に期日が来る、出題対象モードのみ）
    let review_today =
        crate::db::count_due_by_modes(pool, deck_id, "state = 'review'", modes, due_cutoff).await?;

    // 学習中（learning / relearning かつ期日到来、出題対象モードのみ）
    let learning_today = crate::db::count_due_by_modes(
        pool,
        deck_id,
        "state IN ('learning','relearning')",
        modes,
        due_cutoff,
    )
    .await?;

    // 新規（未導入カード数を、実効上限 = new_limit - 今日導入済み ＋学習量の目安 で頭打ち）
    let new_available: i64 = sqlx::query(
        "SELECT COUNT(*) AS c FROM cards c \
         WHERE c.deck_id = ? AND NOT EXISTS \
           (SELECT 1 FROM srs_records sr WHERE sr.card_id = c.id AND sr.deck_id = c.deck_id)",
    )
    .bind(deck_id)
    .fetch_one(pool)
    .await?
    .get("c");
    let calc = crate::db::effective_new_limit(
        pool,
        deck_id,
        modes,
        due_cutoff,
        crate::util::today_jst(),
        new_limit,
        review_limit,
        study_target,
    )
    .await?;
    let new_today = new_available.min(calc.effective);
    // 学習量の目安が有効で、かつ従来の日次新規上限(base)より実効値が小さい＝スロットルが効いている。
    let base = (new_limit - calc.introduced_today).max(0);
    let new_limited_by_study = calc.review_load.is_some() && calc.effective < base;

    Ok((card_count, new_today, review_today, learning_today, new_limited_by_study))
}

fn deck_from_row(
    row: &sqlx::sqlite::SqliteRow,
    card_count: i64,
    new_today: i64,
    review_today: i64,
    learning_today: i64,
    new_limited_by_study: bool,
) -> Deck {
    let test_modes: String = row.get("test_modes");
    Deck {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        language: row
            .try_get::<Option<String>, _>("language")
            .ok()
            .flatten()
            .unwrap_or_else(|| "zh".into()),
        test_modes: serde_json::from_str(&test_modes).unwrap_or_default(),
        daily_new_limit: row.try_get("daily_new_limit").unwrap_or(20),
        daily_review_limit: row.try_get("daily_review_limit").unwrap_or(100),
        daily_study_target: row
            .try_get::<Option<i64>, _>("daily_study_target")
            .ok()
            .flatten(),
        fsrs_target_retention: row.try_get("fsrs_target_retention").unwrap_or(0.90),
        fsrs_max_interval_days: row.try_get("fsrs_max_interval_days").unwrap_or(365),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        card_count,
        new_today,
        review_today,
        learning_today,
        new_limited_by_study,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_decks(db: tauri::State<'_, Db>) -> AppResult<Vec<Deck>> {
    crate::log!(LogLevel::DEBUG, "get_decks");
    let pool = db.inner();
    let due_cutoff = crate::util::logical_day_end_rfc3339();
    let listening_on = crate::db::listening_enabled(pool).await;
    let rows = sqlx::query("SELECT * FROM decks ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.get("id");
        let modes = crate::db::effective_modes(&row.get::<String, _>("test_modes"), listening_on);
        let (cc, nt, rt, lt, lim) = deck_counts(pool, &id, &modes, &due_cutoff).await?;
        out.push(deck_from_row(row, cc, nt, rt, lt, lim));
    }
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn get_deck(db: tauri::State<'_, Db>, deck_id: String) -> AppResult<Deck> {
    crate::log!(LogLevel::DEBUG, "get_deck: {}", deck_id);
    let pool = db.inner();
    let due_cutoff = crate::util::logical_day_end_rfc3339();
    let listening_on = crate::db::listening_enabled(pool).await;
    let row = sqlx::query("SELECT * FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let modes = crate::db::effective_modes(&row.get::<String, _>("test_modes"), listening_on);
    let (cc, nt, rt, lt, lim) = deck_counts(pool, &deck_id, &modes, &due_cutoff).await?;
    Ok(deck_from_row(&row, cc, nt, rt, lt, lim))
}

#[tauri::command]
#[specta::specta]
pub async fn create_deck(db: tauri::State<'_, Db>, input: CreateDeckInput) -> AppResult<Deck> {
    let pool = db.inner();
    validate_id(&input.id, "デッキID")?;

    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM decks WHERE id = ?")
        .bind(&input.id)
        .fetch_optional(pool)
        .await?;
    if exists.is_some() {
        return Err(AppError::Validation(format!(
            "デッキID '{}' は既に存在します",
            input.id
        )));
    }
    if input.test_modes.is_empty() {
        return Err(AppError::Validation(
            "テストモードを1つ以上選択してください".into(),
        ));
    }
    validate_study_target(input.daily_study_target)?;

    let now = now_rfc3339();
    sqlx::query(
        "INSERT INTO decks \
         (id, name, description, language, test_modes, daily_new_limit, daily_review_limit, \
          daily_study_target, fsrs_target_retention, fsrs_max_interval_days, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&input.id)
    .bind(&input.name)
    .bind(&input.description)
    .bind(&input.language)
    .bind(vec_to_json(&input.test_modes))
    .bind(input.daily_new_limit)
    .bind(input.daily_review_limit)
    .bind(input.daily_study_target)
    .bind(input.fsrs_target_retention)
    .bind(input.fsrs_max_interval_days)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    crate::log!(LogLevel::INFO, "Deck created: {} ({})", input.id, input.name);
    get_deck(db, input.id).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_deck(
    db: tauri::State<'_, Db>,
    deck_id: String,
    input: UpdateDeckInput,
) -> AppResult<Deck> {
    let pool = db.inner();
    if input.test_modes.is_empty() {
        return Err(AppError::Validation(
            "テストモードを1つ以上選択してください".into(),
        ));
    }
    validate_study_target(input.daily_study_target)?;
    let now = now_rfc3339();
    let res = sqlx::query(
        "UPDATE decks SET \
         name = ?, description = ?, language = ?, test_modes = ?, \
         daily_new_limit = ?, daily_review_limit = ?, daily_study_target = ?, \
         fsrs_target_retention = ?, fsrs_max_interval_days = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(&input.language)
    .bind(vec_to_json(&input.test_modes))
    .bind(input.daily_new_limit)
    .bind(input.daily_review_limit)
    .bind(input.daily_study_target)
    .bind(input.fsrs_target_retention)
    .bind(input.fsrs_max_interval_days)
    .bind(&now)
    .bind(&deck_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "デッキが見つかりません: {deck_id}"
        )));
    }
    crate::log!(LogLevel::INFO, "Deck updated: {}", deck_id);
    get_deck(db, deck_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_deck(db: tauri::State<'_, Db>, deck_id: String) -> AppResult<()> {
    let pool = db.inner();
    // ON DELETE CASCADE により cards / srs_records / review_logs も連鎖削除される (spec §12)
    let res = sqlx::query("DELETE FROM decks WHERE id = ?")
        .bind(&deck_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "デッキが見つかりません: {deck_id}"
        )));
    }
    crate::log!(LogLevel::INFO, "Deck deleted: {}", deck_id);
    Ok(())
}
