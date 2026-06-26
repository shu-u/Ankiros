// 実行プラットフォーム判定。
// Android WebView の userAgent には必ず "Android" が含まれる。
// 依存追加（@tauri-apps/plugin-os）なしで UI 出し分けに使う軽量判定。
// デスクトップ（Windows 等）では false を返すため、デスクトップ挙動には一切影響しない。
export const isAndroid = (): boolean =>
  typeof navigator !== "undefined" && /android/i.test(navigator.userAgent);
