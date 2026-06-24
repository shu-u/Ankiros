use serde::{Deserialize, Serialize};
use specta::Type;

// ============================================================
// デッキ
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: String,
    pub test_modes: Vec<String>,
    pub daily_new_limit: i64,
    pub daily_review_limit: i64,
    pub fsrs_target_retention: f64,
    pub fsrs_max_interval_days: i64,
    pub created_at: String,
    pub updated_at: String,
    /// カード総数（一覧表示用に算出）
    pub card_count: i64,
    /// 今日の復習予定数（一覧表示用に算出）
    pub due_today: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct CreateDeckInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: String,
    pub test_modes: Vec<String>,
    pub daily_new_limit: i64,
    pub daily_review_limit: i64,
    pub fsrs_target_retention: f64,
    pub fsrs_max_interval_days: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct UpdateDeckInput {
    pub name: String,
    pub description: Option<String>,
    pub language: String,
    pub test_modes: Vec<String>,
    pub daily_new_limit: i64,
    pub daily_review_limit: i64,
    pub fsrs_target_retention: f64,
    pub fsrs_max_interval_days: i64,
}

// ============================================================
// カード
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExampleSentence {
    pub text: String,
    #[serde(default)]
    pub pinyin: String,
    #[serde(default)]
    pub translation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Card {
    pub id: String,
    pub deck_id: String,
    pub hanzi: String,
    pub pinyin_accepted: Vec<String>,
    pub meaning: String,
    pub example_sentences: Vec<ExampleSentence>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub tags: Vec<String>,
    pub ai_notes: Option<String>,
    pub user_notes: String,
    pub audio_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// カード一覧用の軽量型（モードごとのSRS状態を含む）
#[derive(Debug, Clone, Serialize, Type)]
pub struct CardSummary {
    pub id: String,
    pub deck_id: String,
    pub hanzi: String,
    pub meaning: String,
    pub pinyin_accepted: Vec<String>,
    pub tags: Vec<String>,
    /// デッキの各テストモードに対する状態 ('new'|'learning'|'review'|'relearning')
    pub srs_states: Vec<ModeState>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ModeState {
    pub mode: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct CardFilter {
    pub search_text: Option<String>,
    pub tags: Option<Vec<String>>,
    pub srs_state: Option<String>,
}

// ============================================================
// SRS
// ============================================================

#[derive(Debug, Clone, Serialize, Type)]
pub struct SrsRecord {
    pub card_id: String,
    pub deck_id: String,
    pub mode: String,
    pub due_date: String,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub state: String,
    pub reps: i64,
    pub lapses: i64,
    pub last_review: Option<String>,
    pub scheduled_days: i64,
    pub elapsed_days: i64,
}

// ============================================================
// 学習セッション
// ============================================================

#[derive(Debug, Clone, Serialize, Type)]
pub struct SessionCard {
    pub card: Card,
    pub mode: String,
    pub srs_state: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SubmitReviewResult {
    pub updated_srs: SrsRecord,
    /// new_due_date の日付(JST) == 今日(JST) の場合 true
    pub should_requeue: bool,
    /// 各評価ボタンに表示する次回予定（"今日中" / "N日後"）
    pub interval_preview: IntervalPreview,
}

/// 答えフェーズで各評価ボタンに表示する次回間隔プレビュー
#[derive(Debug, Clone, Serialize, Type)]
pub struct IntervalPreview {
    pub again: String,
    pub hard: String,
    pub good: String,
    pub easy: String,
}

// ============================================================
// インポート
// ============================================================

#[derive(Debug, Clone, Serialize, Type)]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
}

// ============================================================
// 統計・アプリ状態
// ============================================================

#[derive(Debug, Clone, Serialize, Type)]
pub struct HomeStats {
    pub streak_days: u32,
    pub today_reviewed: u32,
    pub deck_due_counts: Vec<DeckDueCount>,
    pub seven_day_forecast: Vec<DayForecast>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DeckDueCount {
    pub deck_id: String,
    pub deck_name: String,
    pub due_count: i64,
    pub completed_today: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DayForecast {
    /// JST の日付 (YYYY-MM-DD)
    pub date: String,
    pub count: i64,
    /// 過去の実績（review_logs 由来）なら true、未来の予定なら false
    pub is_past: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct AppStateData {
    pub theme: String,
    pub last_used_deck_id: Option<String>,
    pub window_width: i64,
    pub window_height: i64,
    pub window_x: Option<i64>,
    pub window_y: Option<i64>,
}

// ============================================================
// JSON ファイル (deck.json / batch_NNN.json) のパース用
// ============================================================

#[derive(Debug, Deserialize)]
pub struct DeckJson {
    pub schema_version: String,
    pub deck_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
    pub settings: DeckJsonSettings,
}

fn default_language() -> String {
    "zh".to_string()
}

#[derive(Debug, Deserialize)]
pub struct DeckJsonSettings {
    pub test_modes: Vec<String>,
    #[serde(default = "default_new_limit")]
    pub daily_new_limit: i64,
    #[serde(default = "default_review_limit")]
    pub daily_review_limit: i64,
    #[serde(default)]
    pub fsrs: DeckJsonFsrs,
}

fn default_new_limit() -> i64 {
    20
}
fn default_review_limit() -> i64 {
    100
}

#[derive(Debug, Deserialize)]
pub struct DeckJsonFsrs {
    #[serde(default = "default_retention")]
    pub target_retention: f64,
    #[serde(default = "default_max_interval")]
    pub max_interval_days: i64,
}

impl Default for DeckJsonFsrs {
    fn default() -> Self {
        Self {
            target_retention: default_retention(),
            max_interval_days: default_max_interval(),
        }
    }
}

fn default_retention() -> f64 {
    0.90
}
fn default_max_interval() -> i64 {
    365
}

/// batch_NNN.json の各カード
#[derive(Debug, Deserialize)]
pub struct CardJson {
    pub id: String,
    pub hanzi: String,
    pub pinyin_accepted: Vec<String>,
    pub meaning: String,
    #[serde(default)]
    pub example_sentences: Vec<ExampleSentence>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub antonyms: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// JSON の "notes" → DB の ai_notes へマッピング (spec §4.2/§9)
    #[serde(default)]
    pub notes: Option<String>,
}
