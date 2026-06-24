import { commands, type AppError, type Result } from "@/bindings";

/** AppError を日本語メッセージへ変換する */
export function appErrorMessage(e: AppError): string {
  if ("Validation" in e) return e.Validation;
  if ("NotFound" in e) return `見つかりません: ${e.NotFound}`;
  if ("Database" in e) return `データベースエラー: ${e.Database}`;
  if ("Io" in e) return `IOエラー: ${e.Io}`;
  return "不明なエラーが発生しました";
}

/** Result<T, AppError> を unwrap し、エラー時は Error を throw する */
export async function call<T>(
  promise: Promise<Result<T, AppError>>,
): Promise<T> {
  const res = await promise;
  if (res.status === "error") {
    throw new Error(appErrorMessage(res.error));
  }
  return res.data;
}

export { commands };
