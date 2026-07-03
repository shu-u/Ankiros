use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    pub text: String,
    /// BCP-47 風タグ ("zh-CN" / "ja-JP" / "en-US")。未指定なら端末既定。
    pub lang: Option<String>,
    /// 読み上げ音量 (0.0〜1.0)。未指定なら端末既定 (1.0)。
    pub volume: Option<f32>,
    /// 読み上げ速度 (1.0 = 等速)。未指定なら端末既定 (1.0)。リスニングモードの低速再生用。
    pub rate: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableResponse {
    pub available: bool,
}
