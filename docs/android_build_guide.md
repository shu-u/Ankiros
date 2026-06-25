# Android 実機インストール手順書

## 背景・経緯

別環境（Kazuki PC）で `npm run tauri android dev` を実行した際にエラーが発生した。
その後、ビルドしたAPKを手動で実機転送してインストールしようとしたが、それも失敗した。
このドキュメントはその原因と、実機に恒久的にアプリを入れる手順をまとめたもの。

---

## 発生したエラーと原因

### エラー① `adb install` 失敗（`tauri android dev` 実行時）

```
adb.exe: failed to install app-x86_64-debug.apk
```

**原因：** エミュレーター（`emulator-5554`）が起動していないか、adb が接続できていない状態でインストールしようとした。
ビルド自体（Gradle、Rust）は正常に完了していたため、問題はインストール段階のみ。

---

### エラー② `App not installed as package appears to be invalid`（手動インストール時）

**原因：アーキテクチャの不一致**

| 項目 | 内容 |
|------|------|
| ビルドされたAPK | `app-x86_64-debug.apk`（x86_64向け） |
| ビルド時の選択デバイス | `sdk_gphone16k_x86_64`（x86_64エミュレーター） |
| 物理Android端末のアーキテクチャ | ARM（arm64-v8a） |

x86_64向けのAPKはARMデバイスにインストールできない。

---

## 方法の選択

### `tauri android dev`（USBデバッグ接続）はダメなのか？

| 項目 | 内容 |
|------|------|
| USB切断後も使えるか | **使えない** |
| 理由 | PCのViteデバッグサーバーに依存しているため |
| 向いている用途 | 開発中のリアルタイム動作確認 |

→ **自分だけが使う端末に恒久的にインストールしたい場合は、署名付きリリースAPKのビルドが必要。**

---

## 実機インストールまでの手順

### 前提条件（Kazuki PCに必要なもの）

- Android Studio（インストール済み → JDKも同梱されている）
- Tauri Android ビルド環境（セットアップ済み）

---

### Step 1：キーストアの生成（初回のみ）

APKに署名するための鍵ファイルを生成する。一度作れば使い回せる。

**keytoolがPATHに通っている場合：**

```powershell
keytool -genkey -v -keystore C:\Users\Kazuki\keys\my-release-key.jks -keyalg RSA -keysize 2048 -validity 10000 -alias my-key-alias
```

**PATHに通っていない場合（フルパス指定）：**

```powershell
& "C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe" -genkey -v -keystore C:\Users\Kazuki\keys\my-release-key.jks -keyalg RSA -keysize 2048 -validity 10000 -alias my-key-alias
```

**保存場所の注意：**
- プロジェクトフォルダ内には置かない（Gitに誤って含まれるリスクがある）
- 例：`C:\Users\Kazuki\keys\` など、プロジェクト外の安全な場所に保管

**対話形式の入力内容：**

```
Enter keystore password:       ← パスワードを設定（8文字以上）、忘れずにメモ
Re-enter new password:         ← 同じパスワードを入力
What is your first and last name?      ← 何でもOK（例: Kazuki）
What is your organizational unit?     ← Enterでスキップ可
What is your organization?            ← Enterでスキップ可
What is your City or Locality?        ← Enterでスキップ可
What is your State or Province?       ← Enterでスキップ可
What is your two-letter country code? ← JP
Is CN=Kazuki, ... correct?            ← yes
Enter key password for my-key-alias:  ← Enterで keystore と同じパスワードになる
```

---

### Step 2：署名付きAPKのビルド

環境変数の設定とビルドは**同じPowerShellセッション内**で実行する。

```powershell
$env:ANDROID_KEYSTORE_PATH     = "C:\Users\Kazuki\keys\my-release-key.jks"
$env:ANDROID_KEYSTORE_PASSWORD = "Step1で設定したパスワード"
$env:ANDROID_KEY_ALIAS         = "my-key-alias"
$env:ANDROID_KEY_PASSWORD      = "Step1で設定したパスワード"

cd C:\Users\Kazuki\source\repos\shu-u\Ankiros
npm run tauri android build --target aarch64
```

**毎回打つのが面倒な場合：** プロジェクトルートにスクリプトとして保存しておく。

```powershell
# build-android.ps1 として保存
$env:ANDROID_KEYSTORE_PATH     = "C:\Users\Kazuki\keys\my-release-key.jks"
$env:ANDROID_KEYSTORE_PASSWORD = "パスワード"
$env:ANDROID_KEY_ALIAS         = "my-key-alias"
$env:ANDROID_KEY_PASSWORD      = "パスワード"
npm run tauri android build --target aarch64
```

次回からは `.\build-android.ps1` を実行するだけでOK。

**注意：** `build-android.ps1` はパスワードを含むため、`.gitignore` に追加してGitに含めないこと。

```
# .gitignore に追記
build-android.ps1
```

---

### Step 3：生成されたAPKを確認

ビルド成功後、以下のパスにAPKが生成される：

```
src-tauri/gen/android/app/build/outputs/apk/arm64-v8a/release/app-arm64-v8a-release.apk
```

---

### Step 4：APKをAndroid端末に転送

以下のいずれかの方法で端末に転送する：

- USBケーブルでPCと接続してファイルコピー
- Google Drive / Dropbox などのクラウドストレージ経由
- メール添付（ファイルサイズが小さい場合）

---

### Step 5：Android端末でインストール

**事前設定（初回のみ）：**

「提供元不明のアプリ」のインストールを許可する必要がある。

```
設定 → セキュリティ → 不明なアプリのインストール
→ インストールに使うアプリ（ファイルマネージャーやブラウザ等）を選んで許可
```

設定メニューの場所はAndroidのバージョンやメーカーによって異なる場合がある。

**インストール：**

転送したAPKファイルをファイルマネージャーで開いてインストール。
インストール後はUSB接続やPCなしで通常のアプリとして使用可能。

---

## 更新時の手順

アプリを更新したい場合は **Step 2 のビルド → Step 4 の転送 → Step 5 のインストール** を繰り返すだけでOK。
同じキーストアで署名されていれば、既存のアプリを上書きインストールできる（データも保持される）。

キーストアの生成（Step 1）は初回のみ。

---

## まとめ：残りの作業

| ステップ | 作業内容 | 状況 |
|----------|----------|------|
| Step 1 | キーストア生成 | 未実施（Kazuki PCで1回だけ実行） |
| Step 2 | 署名付きAPKビルド | 未実施（Kazuki PCで実行） |
| Step 3 | APKファイルの確認 | Step 2 完了後 |
| Step 4 | 端末への転送 | Step 3 完了後 |
| Step 5 | 端末の設定変更 & インストール | 端末側で実施 |
