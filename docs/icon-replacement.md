# アプリアイコンの差し替え手順

本プロジェクトは Tauri アプリのため、アイコンは Tauri 付属の `tauri icon` コマンドで一括生成・差し替えるのが最も確実です。各サイズを手動で置き換える必要はありません。

## 1. 用意する画像

| 項目 | 推奨 |
|---|---|
| 形式 | PNG（RGBA = アルファ付き） |
| サイズ | **1024×1024 px**（正方形） |
| 背景 | 透過推奨。ただし Android のアダプティブアイコンは中央に寄せ、周囲に余白（セーフゾーン）を持たせる |

1 枚の元画像から、全プラットフォーム用（Windows の `.ico`、macOS の `.icns`、各種 PNG、**Android の mipmap 一式**）が自動生成されます。

> Android のランチャーは角丸・丸型などに切り抜くため、絵柄を中央 6〜7 割に収め、周囲を透過にしておくと端が切れません。

## 2. 差し替え手順

1. 元画像を用意する（例：プロジェクト直下に `app-icon.png` として置く）。

2. Tauri のアイコン生成コマンドを実行する：

   ```powershell
   npm run tauri icon app-icon.png
   ```

   これで以下が上書き再生成されます：

   - `src-tauri/icons/` … デスクトップ用（`32x32.png`〜`icon.ico` / `icon.icns`、Windows ストア用 `Square*Logo.png` など）
   - `src-tauri/gen/android/app/src/main/res/mipmap-*/ic_launcher*.png` … Android 用

3. 反映を確認してビルドする：

   ```powershell
   npm run tauri:dev          # デスクトップで確認
   npm run tauri android dev  # Android で確認（実機 / エミュレータ）
   ```

## 3. 注意点

- `src-tauri/tauri.conf.json` の `bundle.icon` 配列は既存のファイル名と一致しており、`tauri icon` が同じファイル名で上書きするため **設定の変更は不要** です。
- Android のアダプティブアイコンは「前景（`ic_launcher_foreground`）＋背景」の 2 層構造です。`tauri icon` は前景に元画像を流用し、背景は `src-tauri/gen/android/app/src/main/res/drawable/ic_launcher_background.xml`（現状は単色）を使います。
  - 背景色を変えたい場合は `src-tauri/gen/android/app/src/main/res/values/colors.xml` の該当色を編集します。
- 差し替え後は `git diff` で `src-tauri/icons/` と Android の `res/mipmap-*` が変わっていることを確認してコミットしてください。

## 4. 参考

- 使用するのは元画像 1 枚だけです（各サイズはコマンドが自動生成）。
- Tauri 公式ドキュメント: <https://tauri.app/develop/icons/>
