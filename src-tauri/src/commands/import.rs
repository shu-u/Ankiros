use crate::db::{vec_to_json, Db};
use crate::error::{AppError, AppResult};
use crate::log::LogLevel;
use crate::models::*;
use crate::util::now_rfc3339;
use sqlx::{Sqlite, Transaction};
use std::io::Read;
use std::path::{Path, PathBuf};

/// フォルダ内のカードJSONファイル一覧を返す。
/// `{folder}/cards/*.json` を優先し、無ければ `{folder}/*.json`（deck.json除く）。
fn collect_card_files(folder: &Path) -> AppResult<Vec<PathBuf>> {
    let cards_dir = folder.join("cards");
    let dir = if cards_dir.is_dir() { cards_dir } else { folder.to_path_buf() };

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && p.file_name().and_then(|n| n.to_str()) != Some("deck.json")
        })
        .collect();
    files.sort();
    Ok(files)
}

/// 1枚のカードを upsert する (spec §9)。created/updated を返す。
async fn upsert_card(
    tx: &mut Transaction<'_, Sqlite>,
    deck_id: &str,
    c: &CardJson,
    now: &str,
) -> AppResult<bool> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM cards WHERE id = ? AND deck_id = ?")
            .bind(&c.id)
            .bind(deck_id)
            .fetch_optional(&mut **tx)
            .await?;

    let pinyin = vec_to_json(&c.pinyin_accepted);
    let examples = vec_to_json(&c.example_sentences);
    let synonyms = vec_to_json(&c.synonyms);
    let antonyms = vec_to_json(&c.antonyms);
    let tags = vec_to_json(&c.tags);

    if existing.is_some() {
        // 既存: コンテンツのみ上書き。user_notes / srs_records は維持 (spec §9)
        sqlx::query(
            "UPDATE cards SET \
             hanzi = ?, pinyin_accepted = ?, meaning = ?, example_sentences = ?, \
             synonyms = ?, antonyms = ?, tags = ?, ai_notes = ?, updated_at = ? \
             WHERE id = ? AND deck_id = ?",
        )
        .bind(&c.hanzi)
        .bind(&pinyin)
        .bind(&c.meaning)
        .bind(&examples)
        .bind(&synonyms)
        .bind(&antonyms)
        .bind(&tags)
        .bind(&c.notes) // notes → ai_notes
        .bind(now)
        .bind(&c.id)
        .bind(deck_id)
        .execute(&mut **tx)
        .await?;
        Ok(false)
    } else {
        // 新規: ai_notes = notes, user_notes = "", srs_record は作成しない (spec §9)
        sqlx::query(
            "INSERT INTO cards \
             (id, deck_id, hanzi, pinyin_accepted, meaning, example_sentences, \
              synonyms, antonyms, tags, ai_notes, user_notes, audio_path, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '', NULL, ?, ?)",
        )
        .bind(&c.id)
        .bind(deck_id)
        .bind(&c.hanzi)
        .bind(&pinyin)
        .bind(&c.meaning)
        .bind(&examples)
        .bind(&synonyms)
        .bind(&antonyms)
        .bind(&tags)
        .bind(&c.notes) // notes → ai_notes
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Ok(true)
    }
}

/// カードJSONテキスト群（バッチ）をDBへ取り込む。
/// 1バッチ = 1トランザクション (spec §9)。フォルダ取り込み・ZIP取り込みの共通処理。
/// `batches` は (表示名, JSONテキスト) のリスト。
async fn import_card_batches(
    pool: &Db,
    deck_id: &str,
    batches: &[(String, String)],
) -> AppResult<ImportResult> {
    let now = now_rfc3339();
    let mut created = 0u32;
    let mut updated = 0u32;

    for (name, text) in batches {
        let cards: Vec<CardJson> = serde_json::from_str(text).map_err(|e| {
            AppError::Validation(format!("カードJSONの解析に失敗 ({name}): {e}"))
        })?;

        // バッチ単位でトランザクション
        let mut tx = pool.begin().await?;
        for c in &cards {
            if upsert_card(&mut tx, deck_id, c, &now).await? {
                created += 1;
            } else {
                updated += 1;
            }
        }
        tx.commit().await?;
        crate::log!(
            LogLevel::DEBUG,
            "imported batch {} ({} cards)",
            name,
            cards.len()
        );
    }

    Ok(ImportResult { created, updated })
}

/// フォルダ内のカードファイルを読み込み、共通処理 import_card_batches へ委譲する。
async fn import_card_files(
    pool: &Db,
    deck_id: &str,
    files: &[PathBuf],
) -> AppResult<ImportResult> {
    let mut batches: Vec<(String, String)> = Vec::with_capacity(files.len());
    for file in files {
        let text = std::fs::read_to_string(file)?;
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        batches.push((name, text));
    }
    import_card_batches(pool, deck_id, &batches).await
}

/// deck.json の内容バリデーション (spec §4.1)
fn validate_deck_json(dj: &DeckJson) -> AppResult<()> {
    if dj.schema_version != "1" {
        return Err(AppError::Validation(format!(
            "このdeck.jsonのバージョン（{}）はサポートされていません",
            dj.schema_version
        )));
    }
    if dj.settings.test_modes.is_empty() {
        return Err(AppError::Validation(
            "deck.json の test_modes が空です".into(),
        ));
    }
    Ok(())
}

/// deck.json から decks テーブルへ upsert する。
/// 同じ deck_id が存在する場合は設定を上書き、無ければ新規作成。
async fn upsert_deck(pool: &Db, dj: &DeckJson) -> AppResult<()> {
    let now = now_rfc3339();
    let test_modes = vec_to_json(&dj.settings.test_modes);

    let existing_created: Option<(String,)> =
        sqlx::query_as("SELECT created_at FROM decks WHERE id = ?")
            .bind(&dj.deck_id)
            .fetch_optional(pool)
            .await?;

    if let Some((created_at,)) = existing_created {
        sqlx::query(
            "UPDATE decks SET name = ?, description = ?, language = ?, test_modes = ?, \
             daily_new_limit = ?, daily_review_limit = ?, fsrs_target_retention = ?, \
             fsrs_max_interval_days = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&dj.name)
        .bind(&dj.description)
        .bind(&dj.language)
        .bind(&test_modes)
        .bind(dj.settings.daily_new_limit)
        .bind(dj.settings.daily_review_limit)
        .bind(dj.settings.fsrs.target_retention)
        .bind(dj.settings.fsrs.max_interval_days)
        .bind(&now)
        .bind(&dj.deck_id)
        .execute(pool)
        .await?;
        let _ = created_at;
    } else {
        sqlx::query(
            "INSERT INTO decks \
             (id, name, description, language, test_modes, daily_new_limit, daily_review_limit, \
              fsrs_target_retention, fsrs_max_interval_days, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&dj.deck_id)
        .bind(&dj.name)
        .bind(&dj.description)
        .bind(&dj.language)
        .bind(&test_modes)
        .bind(dj.settings.daily_new_limit)
        .bind(dj.settings.daily_review_limit)
        .bind(dj.settings.fsrs.target_retention)
        .bind(dj.settings.fsrs.max_interval_days)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

// ============================================================
// フォルダ取り込み（デスクトップ用。Android では使用しない）
// ============================================================

#[tauri::command]
#[specta::specta]
pub async fn import_deck_folder(db: tauri::State<'_, Db>, folder_path: String) -> AppResult<ImportResult> {
    crate::log!(LogLevel::INFO, "Importing deck folder: {}", folder_path);
    let pool = db.inner();
    let folder = PathBuf::from(&folder_path);

    let deck_json_path = folder.join("deck.json");
    if !deck_json_path.is_file() {
        return Err(AppError::Validation(
            "選択フォルダに deck.json が見つかりません".into(),
        ));
    }

    let text = std::fs::read_to_string(&deck_json_path)?;
    let dj: DeckJson = serde_json::from_str(&text)
        .map_err(|e| AppError::Validation(format!("deck.json の解析に失敗: {e}")))?;

    validate_deck_json(&dj)?;
    upsert_deck(pool, &dj).await?;

    let files = collect_card_files(&folder)?;
    let res = import_card_files(pool, &dj.deck_id, &files).await?;
    crate::log!(
        LogLevel::INFO,
        "Deck import done: deck={}, created={}, updated={}",
        dj.deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

#[tauri::command]
#[specta::specta]
pub async fn import_cards_folder(
    db: tauri::State<'_, Db>,
    deck_id: String,
    folder_path: String,
) -> AppResult<ImportResult> {
    crate::log!(LogLevel::INFO, "Importing cards into {}: {}", deck_id, folder_path);
    let pool = db.inner();
    // デッキ存在確認
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!(
            "デッキが見つかりません: {deck_id}"
        )));
    }

    let folder = PathBuf::from(&folder_path);
    let files = collect_card_files(&folder)?;
    if files.is_empty() {
        return Err(AppError::Validation(
            "選択フォルダにカードJSON (cards/*.json) が見つかりません".into(),
        ));
    }
    let res = import_card_files(pool, &deck_id, &files).await?;
    crate::log!(
        LogLevel::INFO,
        "Cards import done: deck={}, created={}, updated={}",
        deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

// ============================================================
// ZIP 取り込み（クロスプラットフォーム。Android はこちらを使用）
// ============================================================

/// ZIP から取り出した内容。
struct ZipContents {
    /// ルート（または最浅）の deck.json のテキスト。無ければ None。
    deck_json: Option<String>,
    /// カードバッチ (パス名, JSONテキスト) のリスト。パス名昇順。
    card_batches: Vec<(String, String)>,
}

/// ZIP バイト列を展開し、deck.json とカードJSON群を取り出す (§5.2)。
/// - `cards/` ディレクトリ配下の *.json を優先、無ければ deck.json 以外の全 *.json。
/// - 単一フォルダで包まれた zip（例: `my_deck/deck.json`）も最浅の deck.json を採用して許容。
/// - zip slip 対策として enclosed_name() で安全なエントリ名のみ扱う。
fn extract_zip(bytes: &[u8]) -> AppResult<ZipContents> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| AppError::Validation(format!("ZIPの読み込みに失敗: {e}")))?;

    // (パス, テキスト)
    let mut deck_json: Option<(String, String)> = None;
    let mut json_files: Vec<(String, String)> = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::Validation(format!("ZIPエントリの読み込みに失敗: {e}")))?;
        if file.is_dir() {
            continue;
        }
        let path = match file.enclosed_name() {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue, // 危険なパス（../ 等）はスキップ
        };
        let base = path.rsplit('/').next().unwrap_or("");
        if !base.to_ascii_lowercase().ends_with(".json") {
            continue;
        }

        let mut text = String::new();
        file.read_to_string(&mut text)
            .map_err(|e| AppError::Validation(format!("ZIP内ファイルの読み込みに失敗 ({base}): {e}")))?;

        if base == "deck.json" {
            // 最も浅い deck.json を採用（タイは先勝ち）
            let depth = path.matches('/').count();
            let replace = match &deck_json {
                Some((p, _)) => depth < p.matches('/').count(),
                None => true,
            };
            if replace {
                deck_json = Some((path, text));
            }
        } else {
            json_files.push((path, text));
        }
    }

    // cards/ セグメントを含む json を優先。無ければ全 json。
    let mut chosen: Vec<(String, String)> = {
        let in_cards: Vec<(String, String)> = json_files
            .iter()
            .filter(|(p, _)| p.split('/').any(|seg| seg == "cards"))
            .cloned()
            .collect();
        if in_cards.is_empty() {
            json_files
        } else {
            in_cards
        }
    };
    chosen.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(ZipContents {
        deck_json: deck_json.map(|(_, t)| t),
        card_batches: chosen,
    })
}

/// ZIP からデッキまるごと取り込む（deck.json 必須）(§5.3)。
#[tauri::command]
#[specta::specta]
pub async fn import_deck_zip(db: tauri::State<'_, Db>, zip_path: String) -> AppResult<ImportResult> {
    crate::log!(LogLevel::INFO, "Importing deck zip: {}", zip_path);
    let pool = db.inner();

    let bytes = std::fs::read(&zip_path)?;
    let contents = extract_zip(&bytes)?;

    let deck_text = contents.deck_json.ok_or_else(|| {
        AppError::Validation("ZIP内に deck.json が見つかりません".into())
    })?;
    let dj: DeckJson = serde_json::from_str(&deck_text)
        .map_err(|e| AppError::Validation(format!("deck.json の解析に失敗: {e}")))?;

    validate_deck_json(&dj)?;
    upsert_deck(pool, &dj).await?;

    let res = import_card_batches(pool, &dj.deck_id, &contents.card_batches).await?;
    crate::log!(
        LogLevel::INFO,
        "Deck zip import done: deck={}, created={}, updated={}",
        dj.deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

/// ZIP から既存デッキへカードのみ取り込む（deck.json 不要）(§5.3)。
#[tauri::command]
#[specta::specta]
pub async fn import_cards_zip(
    db: tauri::State<'_, Db>,
    deck_id: String,
    zip_path: String,
) -> AppResult<ImportResult> {
    crate::log!(LogLevel::INFO, "Importing cards zip into {}: {}", deck_id, zip_path);
    let pool = db.inner();
    // デッキ存在確認
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!(
            "デッキが見つかりません: {deck_id}"
        )));
    }

    let bytes = std::fs::read(&zip_path)?;
    let contents = extract_zip(&bytes)?;
    if contents.card_batches.is_empty() {
        return Err(AppError::Validation(
            "ZIP内にカードJSONが見つかりません".into(),
        ));
    }
    let res = import_card_batches(pool, &deck_id, &contents.card_batches).await?;
    crate::log!(
        LogLevel::INFO,
        "Cards zip import done: deck={}, created={}, updated={}",
        deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

/// ZIP バイト列からデッキまるごと取り込む（Android の content:// URI 対応版）(§5.3, §10.2)。
/// フロントが readFile() でバイト列を読み取り、このコマンドへ渡す。
#[tauri::command]
#[specta::specta]
pub async fn import_deck_zip_bytes(
    db: tauri::State<'_, Db>,
    data: Vec<u8>,
) -> AppResult<ImportResult> {
    crate::log!(LogLevel::INFO, "Importing deck zip from bytes ({} bytes)", data.len());
    let pool = db.inner();

    let contents = extract_zip(&data)?;

    let deck_text = contents.deck_json.ok_or_else(|| {
        AppError::Validation("ZIP内に deck.json が見つかりません".into())
    })?;
    let dj: DeckJson = serde_json::from_str(&deck_text)
        .map_err(|e| AppError::Validation(format!("deck.json の解析に失敗: {e}")))?;

    validate_deck_json(&dj)?;
    upsert_deck(pool, &dj).await?;

    let res = import_card_batches(pool, &dj.deck_id, &contents.card_batches).await?;
    crate::log!(
        LogLevel::INFO,
        "Deck zip bytes import done: deck={}, created={}, updated={}",
        dj.deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

/// ZIP バイト列から既存デッキへカードのみ取り込む（Android の content:// URI 対応版）(§5.3, §10.2)。
#[tauri::command]
#[specta::specta]
pub async fn import_cards_zip_bytes(
    db: tauri::State<'_, Db>,
    deck_id: String,
    data: Vec<u8>,
) -> AppResult<ImportResult> {
    crate::log!(
        LogLevel::INFO,
        "Importing cards zip bytes into {}: {} bytes",
        deck_id,
        data.len()
    );
    let pool = db.inner();
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM decks WHERE id = ?")
        .bind(&deck_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!(
            "デッキが見つかりません: {deck_id}"
        )));
    }

    let contents = extract_zip(&data)?;
    if contents.card_batches.is_empty() {
        return Err(AppError::Validation(
            "ZIP内にカードJSONが見つかりません".into(),
        ));
    }
    let res = import_card_batches(pool, &deck_id, &contents.card_batches).await?;
    crate::log!(
        LogLevel::INFO,
        "Cards zip bytes import done: deck={}, created={}, updated={}",
        deck_id,
        res.created,
        res.updated
    );
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const DECK_JSON: &str = r#"{"schema_version":"1","deck_id":"t","name":"T","settings":{"test_modes":["recognition"]}}"#;
    const CARDS_JSON: &str = r#"[{"id":"a","hanzi":"好","pinyin_accepted":["hao3"],"meaning":"good"},{"id":"b","hanzi":"你","pinyin_accepted":["ni3"],"meaning":"you"}]"#;

    /// メモリ上に ZIP を組み立てる
    fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            for (name, content) in entries {
                zw.start_file(*name, opts).unwrap();
                zw.write_all(content.as_bytes()).unwrap();
            }
            zw.finish().unwrap();
        }
        buf
    }

    #[test]
    fn extract_zip_picks_deck_and_cards() {
        let zip = make_zip(&[("deck.json", DECK_JSON), ("cards/batch_001.json", CARDS_JSON)]);
        let c = extract_zip(&zip).unwrap();
        assert!(c.deck_json.is_some());
        assert_eq!(c.card_batches.len(), 1);
        assert_eq!(c.card_batches[0].0, "cards/batch_001.json");
    }

    #[test]
    fn extract_zip_prefers_cards_dir_and_sorts() {
        // cards/ がある場合、トップレベルの紛れ込み json は無視する
        let zip = make_zip(&[
            ("deck.json", DECK_JSON),
            ("cards/b2.json", "[]"),
            ("cards/b1.json", "[]"),
            ("misc.json", "[]"),
        ]);
        let c = extract_zip(&zip).unwrap();
        assert_eq!(c.card_batches.len(), 2);
        assert!(c.card_batches.iter().all(|(p, _)| p.contains("cards/")));
        // パス名昇順
        assert_eq!(c.card_batches[0].0, "cards/b1.json");
        assert_eq!(c.card_batches[1].0, "cards/b2.json");
    }

    #[test]
    fn extract_zip_allows_wrapped_folder() {
        let zip = make_zip(&[("my_deck/deck.json", DECK_JSON), ("my_deck/cards/b1.json", "[]")]);
        let c = extract_zip(&zip).unwrap();
        assert!(c.deck_json.is_some());
        assert_eq!(c.card_batches.len(), 1);
    }

    #[test]
    fn extract_zip_no_deck_json_is_cards_only() {
        let zip = make_zip(&[("cards/b1.json", CARDS_JSON)]);
        let c = extract_zip(&zip).unwrap();
        assert!(c.deck_json.is_none());
        assert_eq!(c.card_batches.len(), 1);
    }

    /// deck.json 検証 → upsert → カード取り込み → 再取り込み (created/updated) の一連を temp DB で検証
    #[tokio::test]
    async fn import_zip_contents_into_temp_db() {
        let db_path = std::env::temp_dir().join(format!("ankiros_test_{}.db", uuid::Uuid::new_v4()));
        let pool = crate::db::init_pool(&db_path).await.unwrap();

        let zip = make_zip(&[("deck.json", DECK_JSON), ("cards/batch_001.json", CARDS_JSON)]);
        let contents = extract_zip(&zip).unwrap();
        let dj: DeckJson = serde_json::from_str(contents.deck_json.as_ref().unwrap()).unwrap();
        validate_deck_json(&dj).unwrap();
        upsert_deck(&pool, &dj).await.unwrap();

        let res = import_card_batches(&pool, &dj.deck_id, &contents.card_batches).await.unwrap();
        assert_eq!(res.created, 2);
        assert_eq!(res.updated, 0);

        // 同じ内容を再取り込みすると全件 updated（id で upsert）
        let res2 = import_card_batches(&pool, &dj.deck_id, &contents.card_batches).await.unwrap();
        assert_eq!(res2.created, 0);
        assert_eq!(res2.updated, 2);

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
    }
}
