use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::TtsExt;

#[command]
pub(crate) async fn speak<R: Runtime>(
    app: AppHandle<R>,
    text: String,
    lang: Option<String>,
) -> crate::Result<()> {
    app.tts().speak(SpeakRequest { text, lang })
}

#[command]
pub(crate) async fn stop<R: Runtime>(app: AppHandle<R>) -> crate::Result<()> {
    app.tts().stop()
}

#[command]
pub(crate) async fn is_available<R: Runtime>(
    app: AppHandle<R>,
) -> crate::Result<AvailableResponse> {
    app.tts().is_available()
}
