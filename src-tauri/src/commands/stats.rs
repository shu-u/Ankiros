use crate::db::Db;
use crate::error::AppResult;
use crate::log::LogLevel;
use crate::models::*;
use crate::util::{to_jst_date, today_jst};
use chrono::{Duration, NaiveDate, Utc};
use sqlx::Row;
use std::collections::HashSet;

#[tauri::command]
#[specta::specta]
pub async fn get_home_stats(db: tauri::State<'_, Db>) -> AppResult<HomeStats> {
    crate::log!(LogLevel::DEBUG, "get_home_stats");
    let pool = db.inner();
    let today = today_jst();
    let now = Utc::now().to_rfc3339();

    // ---- review_logs から ストリーク / 今日の完了数 ----
    let log_rows = sqlx::query("SELECT reviewed_at FROM review_logs")
        .fetch_all(pool)
        .await?;
    let mut log_dates: HashSet<NaiveDate> = HashSet::new();
    let mut today_reviewed = 0u32;
    for r in &log_rows {
        let at: String = r.get("reviewed_at");
        if let Some(d) = to_jst_date(&at) {
            if d == today {
                today_reviewed += 1;
            }
            log_dates.insert(d);
        }
    }

    // 連続学習日数: 今日(なければ昨日)から遡って連続する日数
    let mut streak_days = 0u32;
    let mut cursor = if log_dates.contains(&today) {
        today
    } else {
        today - Duration::days(1)
    };
    while log_dates.contains(&cursor) {
        streak_days += 1;
        cursor -= Duration::days(1);
    }

    // ---- デッキ別 予定数 / 完了数 ----
    let deck_rows = sqlx::query("SELECT id, name, daily_new_limit, daily_review_limit FROM decks")
        .fetch_all(pool)
        .await?;
    let mut deck_due_counts = Vec::new();
    // 今後7日間に新規導入される見込み枚数（デッキ横断で積み上げ）
    let mut new_buckets = [0i64; 7];
    for d in &deck_rows {
        let deck_id: String = d.get("id");
        let deck_name: String = d.get("name");
        let new_limit: i64 = d.try_get("daily_new_limit").unwrap_or(20);
        let review_limit: i64 = d.try_get("daily_review_limit").unwrap_or(100);

        // 復習（state='review' かつ期日到来）
        let due_reviews: i64 = sqlx::query(
            "SELECT COUNT(*) AS c FROM srs_records \
             WHERE deck_id = ? AND state = 'review' AND due_date <= ?",
        )
        .bind(&deck_id)
        .bind(&now)
        .fetch_one(pool)
        .await?
        .get("c");

        // 学習中（learning / relearning かつ期日到来、同日再出題対象）— 予定とは別カウント
        let learning_count: i64 = sqlx::query(
            "SELECT COUNT(*) AS c FROM srs_records \
             WHERE deck_id = ? AND state IN ('learning','relearning') AND due_date <= ?",
        )
        .bind(&deck_id)
        .bind(&now)
        .fetch_one(pool)
        .await?
        .get("c");

        // 新規利用可能（いずれかのモードで srs_record が無いカード × モード数の概算）
        // ここでは「srs_record が1つも無いカード数」を新規予定の近似とする
        let new_available: i64 = sqlx::query(
            "SELECT COUNT(*) AS c FROM cards c \
             WHERE c.deck_id = ? AND NOT EXISTS \
               (SELECT 1 FROM srs_records sr WHERE sr.card_id = c.id AND sr.deck_id = c.deck_id)",
        )
        .bind(&deck_id)
        .fetch_one(pool)
        .await?
        .get("c");

        // 新規は「今日導入済み」を差し引いた実効上限で頭打ち (日次の新規上限)
        let introduced = crate::db::new_introduced_today(pool, &deck_id, today).await?;
        let effective_new_limit = (new_limit - introduced).max(0);
        let new_count = new_available.min(effective_new_limit);
        let review_count = due_reviews.min(review_limit);

        // 7日先までの新規導入をシミュレート: 今日は実効上限、以降は日次上限ぶんずつ残数から導入
        let mut remaining_new = new_available;
        for (day, bucket) in new_buckets.iter_mut().enumerate() {
            let limit = if day == 0 { effective_new_limit } else { new_limit };
            let intro = remaining_new.min(limit);
            *bucket += intro;
            remaining_new -= intro;
        }

        // 今日の完了数（デッキ別）
        let completed_today_rows = sqlx::query(
            "SELECT reviewed_at FROM review_logs WHERE deck_id = ?",
        )
        .bind(&deck_id)
        .fetch_all(pool)
        .await?;
        let completed_today = completed_today_rows
            .iter()
            .filter_map(|r| to_jst_date(&r.get::<String, _>("reviewed_at")))
            .filter(|d| *d == today)
            .count() as i64;

        deck_due_counts.push(DeckDueCount {
            deck_id,
            deck_name,
            new_count,
            review_count,
            learning_count,
            completed_today,
        });
    }

    // ---- 今後7日間の予定（先読み負荷）----
    // 復習: その日に期日が来る件数（learning/relearning/review）。期限切れは overdue として今日に分離計上。
    let srs_rows = sqlx::query("SELECT due_date FROM srs_records WHERE state != 'new'")
        .fetch_all(pool)
        .await?;
    let mut review_buckets = [0i64; 7];
    let mut overdue = 0i64;
    for r in &srs_rows {
        let dd: String = r.get("due_date");
        if let Some(date) = to_jst_date(&dd) {
            let offset = (date - today).num_days();
            if offset < 0 {
                overdue += 1; // 期限切れ（延滞）は今日のバーに別セグメントで表示
            } else if offset < 7 {
                review_buckets[offset as usize] += 1;
            }
        }
    }
    // 今日〜6日後。未来の reviews はレビュー後の再スケジュール分を含まないため下限値。
    let seven_day_forecast: Vec<DayForecast> = (0..7i64)
        .map(|i| DayForecast {
            date: (today + Duration::days(i)).format("%Y-%m-%d").to_string(),
            reviews: review_buckets[i as usize],
            new_cards: new_buckets[i as usize],
            overdue: if i == 0 { overdue } else { 0 },
        })
        .collect();

    crate::log!(
        LogLevel::DEBUG,
        "get_home_stats: streak={}, today_reviewed={}, decks={}",
        streak_days,
        today_reviewed,
        deck_due_counts.len()
    );
    Ok(HomeStats {
        streak_days,
        today_reviewed,
        deck_due_counts,
        seven_day_forecast,
    })
}
