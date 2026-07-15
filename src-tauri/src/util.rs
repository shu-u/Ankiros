use crate::error::{AppError, AppResult};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
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

/// 「今日(論理日)の終端」= 明日の論理日が始まる瞬間の UTC 時刻。
/// `due_date < この値` ⇔ `logical_date(due) <= 今日`、すなわち「今日中に期日が来る（＝本日の出題対象）」。
///
/// forecast（今後7日間の予定）は due を論理日で集計するのに対し、セッションキューや
/// ホーム/デッキの期日カウントは生の now と比較していた。FSRS は due の時刻を保持するため、
/// 「今日中が期日でも時刻がまだ来ていない」カードが forecast には出るのにセッションに出ない不一致が生じる。
/// この関数を境界に使うことで両者を論理日ベースに揃える。
pub fn logical_day_end(now: DateTime<Utc>) -> DateTime<Utc> {
    // 論理日 L は JST の [L 05:00, (L+1) 05:00) に対応する。今日 L0 の終端は (L0+1) の 05:00 JST。
    let boundary_naive = (logical_date(now) + Duration::days(1))
        .and_hms_opt(DAY_RESET_HOUR as u32, 0, 0)
        .expect("05:00 は常に有効な時刻");
    Tokyo
        .from_local_datetime(&boundary_naive)
        .single()
        .expect("JST に夏時間は無く 05:00 は一意に定まる")
        .with_timezone(&Utc)
}

/// `logical_day_end` を現在時刻で評価し RFC3339 文字列で返す（SQL の due_date 比較用の境界値）。
pub fn logical_day_end_rfc3339() -> String {
    logical_day_end(Utc::now()).to_rfc3339()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    /// 論理日は JST 05:00 で切り替わる。logical_day_end は「今日の論理日の終端＝翌 05:00 JST」を返す。
    /// これを due 比較の境界に使うことで「今日中が期日」のカードを時刻に関係なく本日の出題対象にできる。
    #[test]
    fn logical_day_end_rolls_over_at_5am_jst() {
        // 10:00 JST(=01:00Z) の今日は 翌 05:00 JST(=前日 20:00Z) に終わる
        assert_eq!(logical_day_end(utc("2026-07-15T01:00:00Z")), utc("2026-07-15T20:00:00Z"));
        // 04:00 JST(=前日 19:00Z) は 05:00 リセット前なので前日扱い → 同じ境界
        assert_eq!(logical_day_end(utc("2026-07-15T19:00:00Z")), utc("2026-07-15T20:00:00Z"));
        // 05:00 JST ちょうど(=20:00Z) は新しい論理日の開始 → 翌日の境界へ
        assert_eq!(logical_day_end(utc("2026-07-15T20:00:00Z")), utc("2026-07-16T20:00:00Z"));
    }
}
