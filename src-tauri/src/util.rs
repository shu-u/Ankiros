use crate::error::{AppError, AppResult};
use chrono::Utc;

/// 現在時刻を UTC ISO 8601 (RFC3339) 文字列で返す (spec §14)
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
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
