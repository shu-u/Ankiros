import { useState } from "react";
import { Moon, Sun, Download, Upload, Volume2 } from "lucide-react";
import { useAppStore, type Theme } from "@/store/appStore";
import { useSpeech } from "@/lib/useSpeech";
import { logger } from "@/lib/logger";
import { exportBackupToFile, importBackupFromFile } from "@/lib/backup";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const volume = useAppStore((s) => s.volume);
  const setVolume = useAppStore((s) => s.setVolume);
  const saveVolume = useAppStore((s) => s.saveVolume);
  const { speak, supported: speechSupported } = useSpeech();

  const [busy, setBusy] = useState<null | "export" | "import">(null);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);

  const options: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "ライト", icon: Sun },
    { value: "dark", label: "ダーク", icon: Moon },
  ];

  const handleExport = async () => {
    setBusy("export");
    setBackupMsg(null);
    try {
      const path = await exportBackupToFile();
      setBackupMsg(path ? "エクスポートしました。" : null);
      if (path) void logger.info(`Backup exported: ${path}`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBackupMsg(`エクスポートに失敗しました: ${msg}`);
      void logger.error(`Backup export failed: ${msg}`);
    } finally {
      setBusy(null);
    }
  };

  const handleImport = async () => {
    setBusy("import");
    setBackupMsg(null);
    try {
      const res = await importBackupFromFile();
      if (res) {
        setBackupMsg(
          `インポート完了：デッキ ${res.decks}・カード 新規 ${res.cards_created}/更新 ${res.cards_updated}・` +
            `学習進捗 ${res.srs_imported}・履歴 ${res.logs_imported} 件`,
        );
        void logger.info("Backup imported");
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBackupMsg(`インポートに失敗しました: ${msg}`);
      void logger.error(`Backup import failed: ${msg}`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">設定</h1>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">テーマ</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            {options.map(({ value, label, icon: Icon }) => (
              <Button
                key={value}
                variant={theme === value ? "default" : "outline"}
                onClick={() => {
                  void logger.debug(`Theme changed: ${value}`);
                  void setTheme(value);
                }}
                className={cn("gap-2")}
              >
                <Icon className="h-4 w-4" />
                {label}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">読み上げ音量</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center gap-3">
            <Volume2 className="h-4 w-4 shrink-0 text-muted-foreground" />
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={volume}
              onChange={(e) => setVolume(Number(e.target.value))}
              onPointerUp={(e) => void saveVolume(Number(e.currentTarget.value))}
              onKeyUp={(e) => void saveVolume(Number(e.currentTarget.value))}
              className="flex-1 cursor-pointer accent-primary"
              aria-label="読み上げ音量"
            />
            <span className="w-10 text-right text-sm tabular-nums text-muted-foreground">
              {Math.round(volume * 100)}%
            </span>
          </div>
          <p className="text-sm text-muted-foreground">
            カード読み上げ（TTS）の音量です。Windows・Android 共通で反映されます。
          </p>
          {speechSupported && (
            <Button
              variant="outline"
              size="sm"
              className="gap-2"
              onClick={() => speak("你好", "zh-CN")}
            >
              <Volume2 className="h-4 w-4" />
              試聴
            </Button>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">データのバックアップ</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-muted-foreground">
            全デッキのカードと学習進捗（復習スケジュール・履歴）を 1 つの zip
            ファイルに書き出し／復元します。アプリの再インストールや端末の移行前にエクスポートしておくと安全です。
            インポートは現在のデータへ統合（同じカード・進捗は上書き、無いものは追加）します。
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              className="gap-2"
              disabled={busy !== null}
              onClick={() => void handleExport()}
            >
              <Download className="h-4 w-4" />
              {busy === "export" ? "エクスポート中…" : "エクスポート"}
            </Button>
            <Button
              variant="outline"
              className="gap-2"
              disabled={busy !== null}
              onClick={() => void handleImport()}
            >
              <Upload className="h-4 w-4" />
              {busy === "import" ? "インポート中…" : "インポート"}
            </Button>
          </div>
          {backupMsg && (
            <div className="rounded-md border bg-accent/40 px-4 py-2 text-sm">{backupMsg}</div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
