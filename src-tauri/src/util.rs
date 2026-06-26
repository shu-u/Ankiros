use crate::error::{AppError, AppResult};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Asia::Tokyo;

/// 現在時刻を UTC ISO 8601 (RFC3339) 文字列で返す (spec §14)
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// アプリ内の「1日」が切り替わる時刻（JST の時, 0-23）。
/// 例: 5 の場合、午前5時を境に日付が変わり、深夜0〜5時の学習は前日の扱いになる。
/// TODO: 将来的にデッキ設定 or アプリ設定から変更可能にする。
pub const DAY_RESET_HOUR: i64 = 5;

/// 任意の時刻を「アプリ内の論理的な日付」(JST, リセット時刻考慮) に変換する。
/// リセット時刻ぶん巻き戻してから JST の年月日を取ることで、
/// 午前5時より前は前日として集計される。
pub fn logical_date(dt: DateTime<Utc>) -> NaiveDate {
    (dt.with_timezone(&Tokyo) - Duration::hours(DAY_RESET_HOUR)).date_naive()
}

/// 「今日」の論理日付 (JST, リセット時刻考慮) (spec §14)
pub fn today_jst() -> NaiveDate {
    logical_date(Utc::now())
}

/// RFC3339 文字列を論理日付 (JST, リセット時刻考慮) に変換する。パース失敗時は None。
pub fn to_jst_date(rfc3339: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(rfc3339)
        .ok()
        .map(|d| logical_date(d.with_timezone(&Utc)))
}

/// デッキID / カードID 用バリデーション: 英数字・アンダースコアのみ (spec §8.1)
pub fn validate_id(id: &str, label: &str) -> AppResult<()> {
    if id.is_empty() {
        return Err(AppError::Validation(format!("{label} は空にできません")));
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::Validation(format!(
            "{label} は英数字とアンダースコアのみ使用できます: {id}"
        )));
    }
    Ok(())
}
