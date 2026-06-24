use crate::error::{AppError, AppResult};
use crate::models::{IntervalPreview, SrsRecord};
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Asia::Tokyo;
use rs_fsrs::{Card as FsrsCard, Parameters, Rating, State, FSRS};

// ------------------------------------------------------------
// 文字列 ⇔ enum マッピング (spec §5.5)
// ------------------------------------------------------------

pub fn state_to_str(state: State) -> &'static str {
    match state {
        State::New => "new",
        State::Learning => "learning",
        State::Review => "review",
        State::Relearning => "relearning",
    }
}

pub fn str_to_state(s: &str) -> State {
    match s {
        "learning" => State::Learning,
        "review" => State::Review,
        "relearning" => State::Relearning,
        _ => State::New,
    }
}

pub fn str_to_rating(s: &str) -> AppResult<Rating> {
    match s {
        "again" => Ok(Rating::Again),
        "hard" => Ok(Rating::Hard),
        "good" => Ok(Rating::Good),
        "easy" => Ok(Rating::Easy),
        other => Err(AppError::Validation(format!("unknown rating: {other}"))),
    }
}

fn parse_dt(s: &str) -> AppResult<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc))
}

/// JST 基準で「今日」の年月日が等しいか (spec §14)
pub fn same_jst_day(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.with_timezone(&Tokyo).date_naive() == b.with_timezone(&Tokyo).date_naive()
}

// ------------------------------------------------------------
// DBレコード → fsrs::Card (spec §5.5 演算前)
// ------------------------------------------------------------

fn build_fsrs_card(rec: Option<&SrsRecord>, now: DateTime<Utc>) -> AppResult<FsrsCard> {
    match rec {
        // 新規カード: Card::default() (state = New) を初期値とする (spec §5.5)
        None => Ok(FsrsCard {
            due: now,
            last_review: now,
            ..Default::default()
        }),
        Some(r) => Ok(FsrsCard {
            due: parse_dt(&r.due_date).unwrap_or(now),
            stability: r.stability.unwrap_or(0.0),
            difficulty: r.difficulty.unwrap_or(0.0),
            elapsed_days: r.elapsed_days,
            scheduled_days: r.scheduled_days,
            reps: r.reps as i32,
            lapses: r.lapses as i32,
            state: str_to_state(&r.state),
            last_review: r
                .last_review
                .as_deref()
                .and_then(|s| parse_dt(s).ok())
                .unwrap_or(now),
        }),
    }
}

fn make_fsrs(target_retention: f64, max_interval_days: i64) -> FSRS {
    let params = Parameters {
        request_retention: target_retention,
        maximum_interval: max_interval_days as i32,
        ..Default::default()
    };
    FSRS::new(params)
}

/// fsrs::Card → 書き戻し用 SrsRecord フィールド。
/// scheduled_days が上限を超える場合は丸め、due_date も再計算する (spec §5.5)。
fn fsrs_card_to_record(
    mut c: FsrsCard,
    base: &SrsRecord,
    max_interval_days: i64,
    now: DateTime<Utc>,
) -> SrsRecord {
    if c.scheduled_days > max_interval_days {
        c.scheduled_days = max_interval_days;
        c.due = now + Duration::days(max_interval_days);
    }
    SrsRecord {
        card_id: base.card_id.clone(),
        deck_id: base.deck_id.clone(),
        mode: base.mode.clone(),
        due_date: c.due.to_rfc3339(),
        stability: Some(c.stability),
        difficulty: Some(c.difficulty),
        state: state_to_str(c.state).to_string(),
        reps: c.reps as i64,
        lapses: c.lapses as i64,
        last_review: Some(c.last_review.to_rfc3339()),
        scheduled_days: c.scheduled_days,
        elapsed_days: c.elapsed_days,
    }
}

/// 各評価ボタンに表示する次回間隔ラベルを生成する (spec §6.4)。
/// Again など同日再出題は「今日中」、それ以外は「N日後」。
fn interval_label(due: DateTime<Utc>, now: DateTime<Utc>, is_again: bool) -> String {
    let due_jst = due.with_timezone(&Tokyo).date_naive();
    let now_jst = now.with_timezone(&Tokyo).date_naive();
    let days = (due_jst - now_jst).num_days();
    if days <= 0 {
        if is_again {
            "今日中（再出題）".to_string()
        } else {
            "今日中".to_string()
        }
    } else {
        format!("{days}日後")
    }
}

/// submit_review の中核。指定 rating で演算した更新後レコードと、
/// 4評価それぞれの間隔プレビューを返す。
pub struct SrsComputation {
    pub updated: SrsRecord,
    pub preview: IntervalPreview,
    pub should_requeue: bool,
}

pub fn compute_review(
    existing: Option<&SrsRecord>,
    card_id: &str,
    deck_id: &str,
    mode: &str,
    rating: &str,
    target_retention: f64,
    max_interval_days: i64,
    now: DateTime<Utc>,
) -> AppResult<SrsComputation> {
    let rating_enum = str_to_rating(rating)?;
    let fsrs = make_fsrs(target_retention, max_interval_days);

    let base_record = SrsRecord {
        card_id: card_id.to_string(),
        deck_id: deck_id.to_string(),
        mode: mode.to_string(),
        due_date: now.to_rfc3339(),
        stability: existing.and_then(|r| r.stability),
        difficulty: existing.and_then(|r| r.difficulty),
        state: existing.map(|r| r.state.clone()).unwrap_or_else(|| "new".into()),
        reps: existing.map(|r| r.reps).unwrap_or(0),
        lapses: existing.map(|r| r.lapses).unwrap_or(0),
        last_review: existing.and_then(|r| r.last_review.clone()),
        scheduled_days: existing.map(|r| r.scheduled_days).unwrap_or(0),
        elapsed_days: existing.map(|r| r.elapsed_days).unwrap_or(0),
    };

    let card = build_fsrs_card(existing, now)?;

    // 4評価のプレビュー (repeat = preview 全評価)
    let preview_log = fsrs.repeat(card.clone(), now);
    let mk = |rt: Rating, is_again: bool| -> String {
        preview_log
            .get(&rt)
            .map(|info| interval_label(info.card.due, now, is_again))
            .unwrap_or_default()
    };
    let preview = IntervalPreview {
        again: mk(Rating::Again, true),
        hard: mk(Rating::Hard, false),
        good: mk(Rating::Good, false),
        easy: mk(Rating::Easy, false),
    };

    // 選択された評価で確定
    let info = fsrs.next(card, now, rating_enum);
    let updated = fsrs_card_to_record(info.card.clone(), &base_record, max_interval_days, now);

    // 再キュー判定: 新しい due の日付(JST) == 今日(JST) (spec §6.2)
    let should_requeue = same_jst_day(info.card.due, now);

    Ok(SrsComputation {
        updated,
        preview,
        should_requeue,
    })
}
