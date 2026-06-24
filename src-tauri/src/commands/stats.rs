use crate::db::Db;
use crate::error::AppResult;
use crate::log::LogLevel;
use crate::models::*;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Asia::Tokyo;
use sqlx::Row;
use std::collections::HashSet;

fn today_jst() -> NaiveDate {
    Utc::now().with_timezone(&Tokyo).date_naive()
}

fn to_jst_date(rfc3339: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(rfc3339)
        .ok()
        .map(|d| d.with_timezone(&Tokyo).date_naive())
}

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
    let yesterday = today - Duration::days(1);
    let mut log_dates: HashSet<NaiveDate> = HashSet::new();
    let mut today_reviewed = 0u32;
    let mut yesterday_reviewed = 0i64;
    for r in &log_rows {
        let at: String = r.get("reviewed_at");
        if let Some(d) = to_jst_date(&at) {
            if d == today {
                today_reviewed += 1;
            } else if d == yesterday {
                yesterday_reviewed += 1;
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
    for d in &deck_rows {
        let deck_id: String = d.get("id");
        let deck_name: String = d.get("name");
        let new_limit: i64 = d.try_get("daily_new_limit").unwrap_or(20);
        let review_limit: i64 = d.try_get("daily_review_limit").unwrap_or(100);

        // 復習で期日到来
        let due_reviews: i64 = sqlx::query(
            "SELECT COUNT(*) AS c FROM srs_records \
             WHERE deck_id = ? AND state != 'new' AND due_date <= ?",
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

        let planned = due_reviews.min(review_limit) + new_available.min(new_limit);

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
            due_count: planned,
            completed_today,
        });
    }

    // ---- 7日間の予定枚数 ----
    let srs_rows = sqlx::query("SELECT due_date FROM srs_records WHERE state != 'new'")
        .fetch_all(pool)
        .await?;
    let mut buckets: Vec<i64> = vec![0; 7];
    for r in &srs_rows {
        let dd: String = r.get("due_date");
        if let Some(date) = to_jst_date(&dd) {
            let offset = (date - today).num_days();
            if offset < 0 {
                buckets[0] += 1; // 期限切れは今日に計上
            } else if offset < 7 {
                buckets[offset as usize] += 1;
            }
        }
    }
    // 先頭に昨日の実績（review_logs 由来）、続けて今日〜6日後の予定
    let mut seven_day_forecast: Vec<DayForecast> = Vec::with_capacity(8);
    seven_day_forecast.push(DayForecast {
        date: yesterday.format("%Y-%m-%d").to_string(),
        count: yesterday_reviewed,
        is_past: true,
    });
    seven_day_forecast.extend((0..7).map(|i| DayForecast {
        date: (today + Duration::days(i)).format("%Y-%m-%d").to_string(),
        count: buckets[i as usize],
        is_past: false,
    }));

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
