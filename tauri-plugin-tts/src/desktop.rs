use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Tts<R>> {
    Ok(Tts(app.clone()))
}

/// デスクトップではフロントが Web Speech API を使うため、
/// このネイティブ実装は呼ばれない（ビルドを通すためのスタブ）。
pub struct Tts<R: Runtime>(#[allow(dead_code)] AppHandle<R>);

impl<R: Runtime> Tts<R> {
    pub fn speak(&self, _payload: SpeakRequest) -> crate::Result<()> {
        Err(crate::Error::Other(
            "native TTS is not used on desktop".into(),
        ))
    }

    pub fn stop(&self) -> crate::Result<()> {
        Ok(())
    }

    pub fn is_available(&self) -> crate::Result<AvailableResponse> {
        Ok(AvailableResponse { available: false })
    }
}
