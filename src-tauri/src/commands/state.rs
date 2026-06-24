use crate::db::Db;
use crate::error::AppResult;
use crate::log::LogLevel;
use crate::models::AppStateData;
use sqlx::Row;
use std::collections::HashMap;

async fn load_state_map(pool: &Db) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query("SELECT key, value FROM app_state")
        .fetch_all(pool)
        .await?;
    let mut map = HashMap::new();
    for r in &rows {
        let k: String = r.get("key");
        let v: String = r.get("value");
        map.insert(k, v);
    }
    Ok(map)
}

#[tauri::command]
#[specta::specta]
pub async fn get_app_state(db: tauri::State<'_, Db>) -> AppResult<AppStateData> {
    crate::log!(LogLevel::DEBUG, "get_app_state");
    let pool = db.inner();
    let map = load_state_map(pool).await?;
    Ok(AppStateData {
        theme: map.get("theme").cloned().unwrap_or_else(|| "light".into()),
        last_used_deck_id: map.get("last_used_deck_id").cloned().filter(|s| !s.is_empty()),
        window_width: map
            .get("window_width")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1200),
        window_height: map
            .get("window_height")
            .and_then(|v| v.parse().ok())
            .unwrap_or(800),
        window_x: map.get("window_x").and_then(|v| v.parse().ok()),
        window_y: map.get("window_y").and_then(|v| v.parse().ok()),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn update_app_state(db: tauri::State<'_, Db>, key: String, value: String) -> AppResult<()> {
    crate::log!(LogLevel::DEBUG, "update_app_state: {}={}", key, value);
    let pool = db.inner();
    sqlx::query(
        "INSERT INTO app_state (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&key)
    .bind(&value)
    .execute(pool)
    .await?;
    Ok(())
}
