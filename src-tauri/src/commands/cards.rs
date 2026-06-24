use crate::db::{card_from_row, json_to_vec, Db};
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::util::now_rfc3339;
use sqlx::Row;
use std::collections::HashMap;

#[tauri::command]
#[specta::specta]
pub async fn get_cards(
    db: tauri::State<'_, Db>,
    deck_id: String,
    filter: Option<CardFilter>,
) -> AppResult<Vec<CardSummary>> {
    crate::log!(LogLevel::DEBUG, "get_cards: deck={}", deck_id);
    let pool = db.inner();

    // デッキのテストモード一覧（各カードの状態算出に使用）
    let deck_row = sqlx::query("SELECT test_modes FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("デッキが見つかりません: {deck_id}")))?;
    let modes_json: String = deck_row.get("test_modes");
    let deck_modes: Vec<String> = serde_json::from_str(&modes_json).unwrap_or_default();

    let rows = sqlx::query("SELECT * FROM cards WHERE deck_id = ? ORDER BY id ASC")
        .bind(&deck_id)
        .fetch_all(pool)
        .await?;

    // 全 srs_records を取得し card_id ごとに mode→state へ
    let srs_rows = sqlx::query("SELECT card_id, mode, state FROM srs_records WHERE deck_id = ?")
        .bind(&deck_id)
        .fetch_all(pool)
        .await?;
    let mut srs_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for r in &srs_rows {
        let cid: String = r.get("card_id");
        let mode: String = r.get("mode");
        let state: Option<String> = r.get("state");
        srs_map
            .entry(cid)
            .or_default()
            .insert(mode, state.unwrap_or_else(|| "new".into()));
    }

    let f = filter.unwrap_or(CardFilter {
        search_text: None,
        tags: None,
        srs_state: None,
    });
    let search = f.search_text.as_ref().map(|s| s.to_lowercase());

    let mut out = Vec::new();
    for row in &rows {
        let id: String = row.get("id");
        let hanzi: String = row.get("hanzi");
        let meaning: String = row.get("meaning");
        let pinyin_accepted: Vec<String> = json_to_vec(row.get("pinyin_accepted"));
        let tags: Vec<String> = json_to_vec(row.get("tags"));

        // 検索フィルタ: hanzi / meaning 部分一致
        if let Some(ref q) = search {
            if !hanzi.to_lowercase().contains(q) && !meaning.to_lowercase().contains(q) {
                continue;
            }
        }
        // タグフィルタ (AND条件)
        if let Some(ref want) = f.tags {
            if !want.iter().all(|t| tags.contains(t)) {
                continue;
            }
        }

        // 各デッキモードの状態（レコードが無ければ 'new'）
        let card_srs = srs_map.get(&id);
        let srs_states: Vec<ModeState> = deck_modes
            .iter()
            .map(|m| ModeState {
                mode: m.clone(),
                state: card_srs
                    .and_then(|m2| m2.get(m))
                    .cloned()
                    .unwrap_or_else(|| "new".into()),
            })
            .collect();

        // SRS状態フィルタ: いずれかのモードが該当状態
        if let Some(ref st) = f.srs_state {
            if !srs_states.iter().any(|ms| &ms.state == st) {
                continue;
            }
        }

        out.push(CardSummary {
            id,
            deck_id: deck_id.clone(),
            hanzi,
            meaning,
            pinyin_accepted,
            tags,
            srs_states,
        });
    }
    crate::log!(LogLevel::DEBUG, "get_cards: {} results (deck={})", out.len(), deck_id);
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn get_card(db: tauri::State<'_, Db>, card_id: String, deck_id: String) -> AppResult<Card> {
    crate::log!(LogLevel::DEBUG, "get_card: {}/{}", deck_id, card_id);
    let pool = db.inner();
    let row = sqlx::query("SELECT * FROM cards WHERE id = ? AND deck_id = ?")
        .bind(&card_id)
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("カードが見つかりません: {card_id}")))?;
    Ok(card_from_row(&row))
}

#[tauri::command]
#[specta::specta]
pub async fn update_user_notes(
    db: tauri::State<'_, Db>,
    card_id: String,
    deck_id: String,
    notes: String,
) -> AppResult<()> {
    crate::log!(
        LogLevel::DEBUG,
        "update_user_notes: {}/{} ({} chars)",
        deck_id,
        card_id,
        notes.chars().count()
    );
    let pool = db.inner();
    let now = now_rfc3339();
    let res = sqlx::query("UPDATE cards SET user_notes = ?, updated_at = ? WHERE id = ? AND deck_id = ?")
        .bind(&notes)
        .bind(&now)
        .bind(&card_id)
        .bind(&deck_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "カードが見つかりません: {card_id}"
        )));
    }
    Ok(())
}
