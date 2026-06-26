# TTS（Android ネイティブ読み上げ）残作業・引き継ぎ

> 作成: 2026-06-26 / 対象: 開発者本人（Android SDK 環境あり）
> 関連: [android-port-design.md §11.3](./android-port-design.md)（設計の本文）
>
> **状況**: コード実装は完了済み。**残りは Android 実機/SDK 環境でのビルドと検証のみ。**
> ①ステータスバー余白 ②デッキ一覧ヘッダー は実装＆検証済みで、この引き継ぎは ③TTS だけが対象。

---

## 0. 何が終わっていて、何が残っているか

| 区分 | 状態 |
|---|---|
| 自作プラグイン `tauri-plugin-tts`（Rust + Kotlin） | ✅ 実装済み |
| 本体への配線（Cargo / lib.rs / capability / フロント `useSpeech`） | ✅ 実装済み |
| デスクトップ検証（`cargo check` ×2・`npm run build`） | ✅ 通過。**Windows 版は不変**を確認 |
| **Android 実機ビルド（Gradle 取り込み・APK 生成）** | ⏳ **未（要 SDK 環境）** |
| **実機での動作確認** | ⏳ **未** |

なぜ私（実装側）でビルドまでできないか: Kotlin/Gradle 部分は Android SDK/NDK と
実機（またはエミュレータ）が必要なため。Rust とフロントは検証済み。

---

## 1. 手順（この順で実行）

### Step 1. プラグインを Gradle に取り込ませる

新しいプラグインを追加したので、`gen/android` の Gradle インクルードを更新する必要がある。

```powershell
npm run tauri android init
```

- これで `src-tauri/gen/android/tauri.settings.gradle` に `:tauri-plugin-tts` が追加される
  （`settings.gradle` が `apply from: 'tauri.settings.gradle'` で読み込む構造）。
- ⚠️ `gen/android` を手で編集していた場合は退避してから。基本は再生成で問題ない
  （`gen/android` は生成物。`AndroidManifest.xml` の `<queries>` 等の独自追記があれば確認）。

> 取り込まれたか確認: `src-tauri/gen/android/tauri.settings.gradle` に
> `include ':tauri-plugin-tts'` 相当の行が出ていれば OK。

### Step 2. ビルド

開発確認なら dev、APK を作るなら build。

```powershell
# 実機/エミュレータをつないでホットリロード確認
npm run tauri android dev

# もしくは debug 署名 APK を生成（個人利用はこれが手軽）
npm run tauri android build --apk --debug
```

APK 出力先: `src-tauri/gen/android/app/build/outputs/apk/...`

### Step 3. 実機で確認するチェックリスト

- [ ] 学習画面（Study）に🔊発話ボタンが**表示される**（以前は非表示だった）。
- [ ] 漢字（中国語 zh-CN）をタップ → 中国語で読み上げ。
- [ ] 意味・例文の各ボタンも読み上げ（日本語 ja-JP / 中国語）。
- [ ] 読み上げ中はアイコンがパルス（点滅）し、**完了すると自動で止まる**。
- [ ] 連続タップで前の読み上げが止まり新しい方が再生される（QUEUE_FLUSH）。
- [ ] デッキ一覧の「＋」メニュー、上部ステータスバー余白も併せて確認（①②）。

### Step 4. 音声データが無い場合

端末に対象言語の TTS データが入っていないと無音になることがある。

- Android 設定 → **言語と入力 → 音声合成（テキスト読み上げ）** →
  エンジン（Google 音声サービス等）→ 言語データをインストール（中国語・日本語）。

---

## 2. うまく動かないときの切り分け

| 症状 | 確認ポイント |
|---|---|
| ボタンが出ない | `isAndroid()` が true か（WebView UA に "Android"）。`useSpeech` の Android 分岐で `setSupported(true)` している |
| ボタンは出るが無音 | ① 端末に言語データがあるか（Step 4）② Logcat で `TtsPlugin` / `TextToSpeech` のエラー |
| ビルドで `tts:default` が見つからない | `npm run tauri android init` を再実行したか（Step 1）。プラグインの build script が権限を自動生成する |
| ビルドで `:tauri-plugin-tts` が解決できない | `tauri.settings.gradle` に取り込まれているか。`Cargo.toml` の Android 専用依存 `path = "../tauri-plugin-tts"` が正しいか |
| Kotlin コンパイルエラー | `implementation(project(":tauri-android"))` が解決できているか（init 後に有効化される） |
| リリース APK で動かない（debug は動く） | 難読化。`tauri-plugin-tts/android/proguard-rules.pro` の keep 規則が効いているか。まずは `--debug` で確認 |

Logcat 例:
```powershell
adb logcat | Select-String -Pattern "TtsPlugin|TextToSpeech|Tauri"
```

---

## 3. 関連ファイル（実装済み・参照用）

プラグイン本体:
- `tauri-plugin-tts/src/lib.rs` … プラグイン定義・コマンド登録
- `tauri-plugin-tts/src/mobile.rs` … `register_android_plugin("com.plugin.tts","TtsPlugin")`
- `tauri-plugin-tts/src/desktop.rs` … デスクトップ用スタブ（呼ばれない）
- `tauri-plugin-tts/src/commands.rs` / `models.rs` / `error.rs`
- `tauri-plugin-tts/android/src/main/java/com/plugin/tts/TtsPlugin.kt` … Kotlin 本体
- `tauri-plugin-tts/android/build.gradle.kts` / `AndroidManifest.xml`（`<queries>` TTS_SERVICE）
- `tauri-plugin-tts/permissions/default.toml`

本体への配線:
- `src-tauri/Cargo.toml` … `[target.'cfg(target_os = "android")'.dependencies]`
- `src-tauri/src/lib.rs` … `#[cfg(target_os = "android")]` で `.plugin(tauri_plugin_tts::init())`
- `src-tauri/capabilities/android.json` … `platforms:["android"]` + `tts:default`
- `src/lib/useSpeech.ts` … `isAndroid()` 分岐 → `invoke("plugin:tts|speak"/"stop")` + `addPluginListener`
- `src/lib/platform.ts` … `isAndroid()`

---

## 4. 設計上の不変条件（壊さないこと）

- **デスクトップ（Windows）は Web Speech API のまま。** TTS プラグインは Android 専用依存で、
  Windows ビルドには一切含まれない（`cargo check` の依存ツリーに出ないことで確認済み）。
- TTS の追加・変更は `useSpeech` のインターフェース
  `{ supported, speakingText, speak, stop }` を保つ限り、呼び出し側（Study 等）に波及しない。
- プラグイン名・識別子の対応を変えないこと:
  - JS: `invoke("plugin:tts|...")` / `addPluginListener("tts", ...)`
  - Rust: `Builder::new("tts")` / `register_android_plugin("com.plugin.tts", "TtsPlugin")`
  - Kotlin: `package com.plugin.tts` / `class TtsPlugin`
