use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Tts;
#[cfg(mobile)]
use mobile::Tts;

/// `app.tts()` で TTS API にアクセスするための拡張トレイト。
pub trait TtsExt<R: Runtime> {
    fn tts(&self) -> &Tts<R>;
}

impl<R: Runtime, T: Manager<R>> crate::TtsExt<R> for T {
    fn tts(&self) -> &Tts<R> {
        self.state::<Tts<R>>().inner()
    }
}

/// プラグイン初期化。アプリ側で `.plugin(tauri_plugin_tts::init())` する。
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tts")
        .invoke_handler(tauri::generate_handler![
            commands::speak,
            commands::stop,
            commands::is_available
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let tts = mobile::init(app, api)?;
            #[cfg(desktop)]
            let tts = desktop::init(app, api)?;
            app.manage(tts);
            Ok(())
        })
        .build()
}
