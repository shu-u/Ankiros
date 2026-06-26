// JS から invoke 可能なコマンド名。
// build script がこれらの権限 (allow-speak など) を自動生成し、
// android_path で Android プロジェクトをアプリのビルドへ登録する。
const COMMANDS: &[&str] = &["speak", "stop", "is_available"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
