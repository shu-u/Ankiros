use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::util::{to_jst_date, today_jst};
use chrono::{Duration, NaiveDate};
use sqlx::Row;
use std::collections::{HashMap, HashSet};

/// 「定着（成熟）」とみなす復習間隔のしきい値（日）。Anki 慣例の 21 日。
const MATURE_INTERVAL_DAYS: i64 = 21;

#[tauri::command]
#[specta::specta]
pub async fn get_home_stats(db: tauri::State<'_, Db>) -> AppResult<HomeStats> {
    crate::log!(LogLevel::DEBUG, "get_home_stats");
    let pool = db.inner();
    let today = today_jst();
    // 期日カウントは forecast（論理日ベース）と揃える: due < 明日の論理日開始 = 今日中に期日到来。
    let due_cutoff = crate::util::logical_day_end_rfc3339();

    // 期日カウント・先読みは「デッキの出題対象モード（test_modes、無音環境なら listening 除外）」
    // のみを対象にする。セッションキューが出題するモードと一致させ、test_modes 外の
    // srs_records（過去モードの残骸など）をホームが数えてしまう不一致を防ぐ。
    let listening_on = crate::db::listening_enabled(pool).await;

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
    let deck_rows = sqlx::query("SELECT id, name, test_modes, daily_new_limit, daily_review_limit FROM decks")
        .fetch_all(pool)
        .await?;
    let mut deck_due_counts = Vec::new();
    // 今後7日間に新規導入される見込み枚数（デッキ横断で積み上げ）
    let mut new_buckets = [0i64; 7];
    // 今後7日間の復習見込み（デッキ横断）。期限切れは overdue として今日に分離計上。
    let mut review_buckets = [0i64; 7];
    let mut overdue = 0i64;
    for d in &deck_rows {
        let deck_id: String = d.get("id");
        let deck_name: String = d.get("name");
        let test_modes_json: String = d.get("test_modes");
        let modes = crate::db::effective_modes(&test_modes_json, listening_on);
        let new_limit: i64 = d.try_get("daily_new_limit").unwrap_or(20);
        let review_limit: i64 = d.try_get("daily_review_limit").unwrap_or(100);

        // 復習（state='review' かつ期日到来、出題対象モードのみ）
        let due_reviews =
            crate::db::count_due_by_modes(pool, &deck_id, "state = 'review'", &modes, &due_cutoff)
                .await?;

        // 学習中（learning / relearning かつ期日到来、同日再出題対象）— 予定とは別カウント
        let learning_count = crate::db::count_due_by_modes(
            pool,
            &deck_id,
            "state IN ('learning','relearning')",
            &modes,
            &due_cutoff,
        )
        .await?;

        // 先読み（今後7日間）の復習バケットへ、このデッキの出題対象モードの due を積む。
        if !modes.is_empty() {
            let placeholders = std::iter::repeat("?")
                .take(modes.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT due_date FROM srs_records \
                 WHERE deck_id = ? AND state != 'new' AND mode IN ({placeholders})"
            );
            let mut q = sqlx::query(&sql).bind(&deck_id);
            for m in &modes {
                q = q.bind(m);
            }
            let srs_rows = q.fetch_all(pool).await?;
            for r in &srs_rows {
                let dd: String = r.get("due_date");
                if let Some(date) = to_jst_date(&dd) {
                    let offset = (date - today).num_days();
                    if offset < 0 {
                        overdue += 1;
                    } else if offset < 7 {
                        review_buckets[offset as usize] += 1;
                    }
                }
            }
        }

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
    // 復習バケット（review_buckets）・overdue は上のデッキ別ループで、各デッキの出題対象モードに
    // 限定して積み上げ済み。未来の reviews はレビュー後の再スケジュール分を含まないため下限値。
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

/// デッキ全体の習得度内訳を算出する（デッキ詳細画面の「学習進捗」表示用）。
/// 学習単位は (カード × テストモード)。未学習はレコードが無いユニットなので、
/// カード数から学習済み（learning/review 等）を差し引いて算出する。
#[tauri::command]
#[specta::specta]
pub async fn get_deck_progress(db: tauri::State<'_, Db>, deck_id: String) -> AppResult<DeckProgress> {
    crate::log!(LogLevel::DEBUG, "get_deck_progress: {}", deck_id);
    let pool = db.inner();

    // デッキのテストモード一覧（総ユニット数と未学習の算出に使用）
    let deck_row = sqlx::query("SELECT test_modes FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let modes_json: String = deck_row.get("test_modes");
    let mut deck_modes: Vec<String> = serde_json::from_str(&modes_json).unwrap_or_default();

    // 無音環境トグルが OFF なら listening を進捗の分母・内訳から動的に除外する。
    // srs_records は保持したままなので、ON に戻せば元の進捗に復帰する。
    if !crate::db::listening_enabled(pool).await {
        deck_modes.retain(|m| m != "listening");
    }

    let card_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM cards WHERE deck_id = ?")
        .bind(&deck_id)
        .fetch_one(pool)
        .await?
        .get("c");

    // モード別に 学習中 / 習得中(若い) / 定着(成熟) を集計。
    // MATURE_INTERVAL_DAYS は const i64 なので format! で埋め込んでも安全。
    let sql = format!(
        "SELECT mode, \
         SUM(CASE WHEN state IN ('learning','relearning') THEN 1 ELSE 0 END) AS learning, \
         SUM(CASE WHEN state = 'review' AND scheduled_days <  {t} THEN 1 ELSE 0 END) AS young, \
         SUM(CASE WHEN state = 'review' AND scheduled_days >= {t} THEN 1 ELSE 0 END) AS mature \
         FROM srs_records WHERE deck_id = ? GROUP BY mode",
        t = MATURE_INTERVAL_DAYS
    );
    let rows = sqlx::query(&sql).bind(&deck_id).fetch_all(pool).await?;
    // mode -> (learning, young, mature)
    let mut agg: HashMap<String, (i64, i64, i64)> = HashMap::new();
    for r in &rows {
        let mode: String = r.get("mode");
        let learning: i64 = r.try_get("learning").unwrap_or(0);
        let young: i64 = r.try_get("young").unwrap_or(0);
        let mature: i64 = r.try_get("mature").unwrap_or(0);
        agg.insert(mode, (learning, young, mature));
    }

    let mut modes = Vec::with_capacity(deck_modes.len());
    let (mut t_new, mut t_learning, mut t_young, mut t_mature) = (0i64, 0i64, 0i64, 0i64);
    for m in &deck_modes {
        let (learning, young, mature) = agg.get(m).copied().unwrap_or((0, 0, 0));
        // 未学習 = カード数 − 学習済みユニット（負にはしない）
        let new_count = (card_count - learning - young - mature).max(0);
        t_new += new_count;
        t_learning += learning;
        t_young += young;
        t_mature += mature;
        modes.push(ModeProgress {
            mode: m.clone(),
            new_count,
            learning_count: learning,
            young_count: young,
            mature_count: mature,
        });
    }

    // 今日の完了数（デッキ別、JST 論理日付ベース）
    let today = today_jst();
    let log_rows = sqlx::query("SELECT reviewed_at FROM review_logs WHERE deck_id = ?")
        .bind(&deck_id)
        .fetch_all(pool)
        .await?;
    let completed_today = log_rows
        .iter()
        .filter_map(|r| to_jst_date(&r.get::<String, _>("reviewed_at")))
        .filter(|d| *d == today)
        .count() as i64;

    Ok(DeckProgress {
        total_units: card_count * deck_modes.len() as i64,
        new_count: t_new,
        learning_count: t_learning,
        young_count: t_young,
        mature_count: t_mature,
        modes,
        completed_today,
    })
}
