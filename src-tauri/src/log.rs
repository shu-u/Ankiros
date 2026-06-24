//! # ログレベル使用ガイドライン
//!
//! ## ERROR
//! - 致命的なエラー、システムの動作に重大な影響を与える問題
//! - 例: データベース接続失敗、データ破損、予期しない例外
//!
//! ## WARN
//! - 警告、エラーではないが注意が必要な状況
//! - 例: 非推奨機能の使用、リソース不足の警告
//!
//! ## INFO
//! - ある程度大きな塊での動作情報
//! - 例: サービス初期化、主要処理の開始/完了、問題選択結果
//! - 使用場所: サービスレイヤーの主要な処理単位
//!
//! ## DEBUG
//! - バックエンド、フロントエンド内部の動作情報
//! - 例: 関数の入力パラメータ、処理の中間結果、DB操作の完了通知
//! - 使用場所: デバッグ時に役立つ詳細な処理情報
//!
//! ## VERBOSE
//! - 短時間に大量のログ出力が予想されるもの、DEBUG以上に詳細な値等を長文で出力するもの
//! - 例: ループ内のログ、大きなオブジェクトのダンプ、頻繁に呼ばれる関数の詳細
//! - 使用場所: パフォーマンスに影響を与える可能性がある詳細ログ

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[allow(unused)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Color {
    RED,
    GREEN,
    YELLOW,
    BLUE,
    PURPLE,
    LIGHTBLUE,
    GRAY,
    END,
}

#[allow(unused)]
impl Color {
    pub(crate) fn escape(&self) -> &str {
        match self {
            Self::RED => "\x1b[38;5;1m",
            Self::GREEN => "\x1b[38;5;2m",
            Self::YELLOW => "\x1b[38;5;3m",
            Self::BLUE => "\x1b[38;5;4m",
            Self::PURPLE => "\x1b[38;5;5m",
            Self::LIGHTBLUE => "\x1b[38;5;6m",
            Self::GRAY => "\x1b[38;5;8m",
            Self::END => "\x1b[m",
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::RED => "RED",
            Self::GREEN => "GREEN",
            Self::YELLOW => "YELLOW",
            Self::BLUE => "BLUE",
            Self::PURPLE => "PURPLE",
            Self::LIGHTBLUE => "LIGHTBLUE",
            Self::GRAY => "GRAY",
            Self::END => "END",
        }
    }
}

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize, specta::Type,
)]
#[allow(unused)]
pub enum LogLevel {
    ERROR,
    WARN,
    INFO,
    DEBUG,
    VERBOSE,
}

impl LogLevel {
    pub(crate) fn to_string(&self) -> String {
        match self {
            LogLevel::ERROR => "ERROR".to_string(),
            LogLevel::WARN => "WARN".to_string(),
            LogLevel::INFO => "INFO".to_string(),
            LogLevel::DEBUG => "DEBUG".to_string(),
            LogLevel::VERBOSE => "VERBOSE".to_string(),
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ERROR" => Some(LogLevel::ERROR),
            "WARN" => Some(LogLevel::WARN),
            "INFO" => Some(LogLevel::INFO),
            "DEBUG" => Some(LogLevel::DEBUG),
            "VERBOSE" => Some(LogLevel::VERBOSE),
            _ => None,
        }
    }
}

// グローバルな最小ログレベル
static MIN_LOG_LEVEL: OnceLock<LogLevel> = OnceLock::new();

/// ログレベルを初期化する
/// 環境変数 TAURI_LOG_LEVEL から読み取る
/// 未設定の場合は VERBOSE（すべてのログを表示）
pub fn init_log_level() {
    let level = std::env::var("TAURI_LOG_LEVEL")
        .ok()
        .and_then(|s| LogLevel::from_str(&s))
        .unwrap_or(LogLevel::VERBOSE);

    MIN_LOG_LEVEL.set(level).ok();

    println!(
        "{}Log level initialized: {}{}",
        Color::LIGHTBLUE.escape(),
        level.to_string(),
        Color::END.escape()
    );
}

/// 指定されたログレベルがフィルタを通過するかチェック
pub(crate) fn should_log(level: LogLevel) -> bool {
    let min_level = MIN_LOG_LEVEL.get().copied().unwrap_or(LogLevel::VERBOSE);
    level <= min_level
}

pub fn pad_string(str: String, len: usize) -> String {
    let mut r = str;
    while r.len() < len {
        r.push(' ');
    }
    r
}

#[tauri::command]
#[specta::specta]
pub fn log(log_level: LogLevel, log: String) {
    // フロントエンドのログも TAURI_LOG_LEVEL のフィルタを尊重する
    // （元実装ではここでフィルタしておらず、レベル設定がFEログに効かなかったため改善）
    if !should_log(log_level) {
        return;
    }
    let log_color = match log_level {
        LogLevel::ERROR => Color::RED.escape(),
        LogLevel::WARN => Color::YELLOW.escape(),
        LogLevel::INFO => "",
        LogLevel::DEBUG => "",
        LogLevel::VERBOSE => Color::GRAY.escape(),
    };
    let log_end = if log_level == LogLevel::INFO {
        ""
    } else {
        Color::END.escape()
    };
    println!(
        "{}FE  {}: {}({}) {}{}",
        Color::LIGHTBLUE.escape(),
        Color::END.escape(),
        log_color,
        pad_string(log_level.to_string(), LogLevel::VERBOSE.to_string().len()),
        log,
        log_end
    );
}

#[allow(unused)]
#[macro_export]
macro_rules! log {
    ($log_level:expr, $($log:expr),+) => {{
        use $crate::log::{Color, pad_string, should_log};

        if should_log($log_level) {
            let log_color = match $log_level {
                LogLevel::ERROR => Color::RED.escape(),
                LogLevel::WARN => Color::YELLOW.escape(),
                LogLevel::INFO => "",
                LogLevel::DEBUG => "",
                LogLevel::VERBOSE => Color::GRAY.escape()
            };
            let log_end = if $log_level == LogLevel::INFO {
                ""
            } else {
                Color::END.escape()
            };

            println!("{}Rust{}: {}({}) {}{}", Color::PURPLE.escape(), Color::END.escape(), log_color, pad_string($log_level.to_string(), LogLevel::VERBOSE.to_string().len()), format!($($log),+), log_end);
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::LogLevel;

    #[test]
    fn test_log_level_ordering() {
        // ログレベルの順序が正しいことを確認
        assert!(LogLevel::ERROR < LogLevel::WARN);
        assert!(LogLevel::WARN < LogLevel::INFO);
        assert!(LogLevel::INFO < LogLevel::DEBUG);
        assert!(LogLevel::DEBUG < LogLevel::VERBOSE);
    }

    #[test]
    fn test_log_level_from_str() {
        // 大文字小文字を区別しない
        assert_eq!(LogLevel::from_str("error"), Some(LogLevel::ERROR));
        assert_eq!(LogLevel::from_str("ERROR"), Some(LogLevel::ERROR));
        assert_eq!(LogLevel::from_str("Error"), Some(LogLevel::ERROR));

        // すべてのレベルをテスト
        assert_eq!(LogLevel::from_str("warn"), Some(LogLevel::WARN));
        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::INFO));
        assert_eq!(LogLevel::from_str("debug"), Some(LogLevel::DEBUG));
        assert_eq!(LogLevel::from_str("verbose"), Some(LogLevel::VERBOSE));

        // 無効な値
        assert_eq!(LogLevel::from_str("invalid"), None);
        assert_eq!(LogLevel::from_str(""), None);
    }

    #[test]
    fn test_log_level_to_string() {
        assert_eq!(LogLevel::ERROR.to_string(), "ERROR");
        assert_eq!(LogLevel::WARN.to_string(), "WARN");
        assert_eq!(LogLevel::INFO.to_string(), "INFO");
        assert_eq!(LogLevel::DEBUG.to_string(), "DEBUG");
        assert_eq!(LogLevel::VERBOSE.to_string(), "VERBOSE");
    }

    #[test]
    fn test_log_filtering_logic() {
        // ERROR レベルの場合
        assert!(LogLevel::ERROR <= LogLevel::ERROR);
        assert!(!(LogLevel::WARN <= LogLevel::ERROR));

        // INFO レベルの場合
        assert!(LogLevel::ERROR <= LogLevel::INFO);
        assert!(LogLevel::INFO <= LogLevel::INFO);
        assert!(!(LogLevel::DEBUG <= LogLevel::INFO));

        // VERBOSE レベルの場合（すべて表示）
        assert!(LogLevel::ERROR <= LogLevel::VERBOSE);
        assert!(LogLevel::VERBOSE <= LogLevel::VERBOSE);
    }
}
