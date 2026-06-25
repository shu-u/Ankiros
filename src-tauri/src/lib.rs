mod commands;
mod db;
mod error;
#[macro_use]
mod log;
mod models;
mod srs;
mod util;

#[cfg(desktop)]
use db::Db;
use log::LogLevel;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// 終了時にウィンドウのサイズ・位置を app_state へ保存する（デスクトップ専用）
#[cfg(desktop)]
fn save_window_state(pool: &Db, win: &tauri::WebviewWindow) {
    log!(LogLevel::DEBUG, "Saving window state (close requested)");
    let scale = win.scale_factor().unwrap_or(1.0);
    if let Ok(size) = win.inner_size() {
        let logical = size.to_logical::<i32>(scale);
        let _ = tauri::async_runtime::block_on(async {
            for (k, v) in [
                ("window_width", logical.width.to_string()),
                ("window_height", logical.height.to_string()),
            ] {
                let _ = sqlx::query(
                    "INSERT INTO app_state (key, value) VALUES (?, ?) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(k)
                .bind(v)
                .execute(pool)
                .await;
            }
            Ok::<(), sqlx::Error>(())
        });
    }
    if let Ok(pos) = win.outer_position() {
        let logical = pos.to_logical::<i32>(scale);
        let _ = tauri::async_runtime::block_on(async {
            for (k, v) in [
                ("window_x", logical.x.to_string()),
                ("window_y", logical.y.to_string()),
            ] {
                let _ = sqlx::query(
                    "INSERT INTO app_state (key, value) VALUES (?, ?) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(k)
                .bind(v)
                .execute(pool)
                .await;
            }
            Ok::<(), sqlx::Error>(())
        });
    }
}

/// 起動時に保存済みのウィンドウサイズ・位置を復元する (spec §3.3 app_state)（デスクトップ専用）
#[cfg(desktop)]
fn restore_window_state(pool: &Db, win: &tauri::WebviewWindow) {
    log!(LogLevel::DEBUG, "Restoring window state");
    let prefs = tauri::async_runtime::block_on(async {
        let rows = sqlx::query_as::<_, (String, String)>("SELECT key, value FROM app_state")
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        rows
    });
    let mut width = 1200i64;
    let mut height = 800i64;
    let mut x: Option<i64> = None;
    let mut y: Option<i64> = None;
    for (k, v) in prefs {
        match k.as_str() {
            "window_width" => width = v.parse().unwrap_or(1200),
            "window_height" => height = v.parse().unwrap_or(800),
            "window_x" => x = v.parse().ok(),
            "window_y" => y = v.parse().ok(),
            _ => {}
        }
    }
    let _ = win.set_size(tauri::LogicalSize::new(width as f64, height as f64));
    // window_x / window_y のキーが無い場合は set_position を呼ばず OS に任せる (spec §3.3)
    if let (Some(x), Some(y)) = (x, y) {
        let _ = win.set_position(tauri::LogicalPosition::new(x as f64, y as f64));
    }
}

/// 全 IPC コマンドを登録した tauri-specta Builder を構築する。
/// run() とバインディング生成テストの両方から使用する。
fn make_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::get_decks,
        commands::get_deck,
        commands::create_deck,
        commands::update_deck,
        commands::delete_deck,
        commands::get_cards,
        commands::get_card,
        commands::update_user_notes,
        commands::import_deck_folder,
        commands::import_cards_folder,
        commands::import_deck_zip,
        commands::import_cards_zip,
        commands::import_deck_zip_bytes,
        commands::import_cards_zip_bytes,
        commands::get_session_queue,
        commands::preview_review,
        commands::submit_review,
        commands::get_home_stats,
        commands::get_app_state,
        commands::update_app_state,
        log::log,
    ])
    .typ::<log::LogLevel>()
}

/// TypeScript エクスポート設定。i64 等は JS の number として出力する。
/// 生成ファイルは noUnusedLocals 等に引っかかるため @ts-nocheck を付与する。
fn ts_config() -> specta_typescript::Typescript {
    specta_typescript::Typescript::default()
        .bigint(specta_typescript::BigIntExportBehavior::Number)
        .header("// @ts-nocheck\n")
}

/// TypeScript バインディングを ../src/bindings.ts へ出力する。
/// `cargo run --bin gen_bindings` から呼ばれる。
pub fn export_typescript_bindings() {
    make_builder()
        .export(ts_config(), "../src/bindings.ts")
        .expect("failed to export typescript bindings");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ログレベルを初期化（環境変数 TAURI_LOG_LEVEL、未設定なら VERBOSE）
    log::init_log_level();

    let builder = make_builder();

    // 開発時に TypeScript バインディングを生成 (tauri-specta)
    #[cfg(debug_assertions)]
    builder
        .export(ts_config(), "../src/bindings.ts")
        .expect("failed to export typescript bindings");

    #[allow(unused_mut)]
    let mut tauri_builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // DB 初期化（app_data/app.db、マイグレーション自動適用）
            let app_data_dir = app.path().app_data_dir()?;
            let db_path = app_data_dir.join("app.db");
            let pool = tauri::async_runtime::block_on(async { db::init_pool(&db_path).await })
                .map_err(|e| {
                    log!(LogLevel::ERROR, "DB初期化に失敗しました: {e:?}");
                    format!("DB初期化に失敗しました: {e:?}")
                })?;
            log!(LogLevel::INFO, "DB initialized: {}", db_path.display());

            // ウィンドウ状態の復元はデスクトップのみ（Android にはウィンドウ概念がない）
            #[cfg(desktop)]
            if let Some(win) = app.get_webview_window("main") {
                restore_window_state(&pool, &win);
            }

            app.manage(pool);
            log!(LogLevel::DEBUG, "Setup complete, app state managed");
            Ok(())
        });

    // ウィンドウ状態の保存はデスクトップのみ（Android にはウィンドウのクローズ概念がない）
    #[cfg(desktop)]
    {
        tauri_builder = tauri_builder.on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(pool) = window.try_state::<Db>() {
                    if let Some(win) = window.get_webview_window("main") {
                        save_window_state(&pool, &win);
                    }
                }
            }
        });
    }

    tauri_builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

