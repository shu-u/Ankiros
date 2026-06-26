// 学習データのバックアップ（エクスポート/インポート）フロント側ロジック。
// Windows / Android 共通。ファイル I/O はプラグインがプラットフォーム差を吸収する:
//  - エクスポート: バックエンドが zip バイト列を返す → save() で保存先を選び writeFile() で書き出す。
//    Android では save() が SAF(ACTION_CREATE_DOCUMENT) を起動し、writeFile() が content:// へ書く。
//  - インポート: open() + readFile() でバイト列を取得（既存のデッキ取り込みと同じ content:// 対応経路）。
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeFile, readFile } from "@tauri-apps/plugin-fs";
import { call, commands } from "@/lib/api";
import type { BackupImportResult } from "@/bindings";

const ZIP_FILTER = [{ name: "バックアップ (zip)", extensions: ["zip"] }];

/** 既定のバックアップファイル名（端末ローカル日付）。例: ankiros_backup_20260626.zip */
function defaultBackupName(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `ankiros_backup_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}.zip`;
}

/**
 * 全学習データを zip にエクスポートし、保存ダイアログで選んだ場所へ書き出す。
 * 戻り値: 保存先パス（成功）/ null（ユーザーがキャンセル）。失敗時は例外を送出。
 */
export async function exportBackupToFile(): Promise<string | null> {
  const bytes = await call(commands.exportBackup());
  const path = await save({
    title: "学習データをエクスポート",
    defaultPath: defaultBackupName(),
    filters: ZIP_FILTER,
  });
  if (!path) return null; // キャンセル
  await writeFile(path, new Uint8Array(bytes));
  return path;
}

/**
 * バックアップ zip を選択し、現在のデータへマージ復元する。
 * 戻り値: 取り込み結果（成功）/ null（ユーザーがキャンセル）。失敗時は例外を送出。
 */
export async function importBackupFromFile(): Promise<BackupImportResult | null> {
  const file = await open({
    multiple: false,
    title: "バックアップを選択",
    filters: ZIP_FILTER,
  });
  if (!file || typeof file !== "string") return null; // キャンセル
  const bytes = await readFile(file);
  // deck_ids = null で全デッキを復元（将来の選択復元に対応した引数）。
  return await call(commands.importBackup(Array.from(bytes), null));
}
