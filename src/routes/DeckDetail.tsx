import { useState } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { FileArchive, FolderInput, List, Pencil, PlayCircle, Trash2 } from "lucide-react";
import type { Deck, DeckProgress, ImportResult } from "@/bindings";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { isAndroid } from "@/lib/platform";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ConfirmDialog } from "@/components/ui/modal";
import { DeckSettingsForm } from "@/components/DeckSettingsForm";
import { Loading, ErrorBox, modeLabel } from "@/components/common";

export function DeckDetailPage() {
  const { deckId } = useParams({ strict: false }) as { deckId: string };
  const navigate = useNavigate();
  const deck = useAsync(() => call(commands.getDeck(deckId)), [deckId]);
  const progress = useAsync(() => call(commands.getDeckProgress(deckId)), [deckId]);
  const [importMsg, setImportMsg] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [editingSettings, setEditingSettings] = useState(false);

  const handleImportCards = async () => {
    const folder = await open({ directory: true, title: "カードフォルダを選択" });
    if (!folder || typeof folder !== "string") return;
    try {
      const res: ImportResult = await call(commands.importCardsFolder(deckId, folder));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      deck.reload();
      progress.reload();
    } catch (e) {
      setImportMsg(e instanceof Error ? e.message : String(e));
    }
  };

  // ZIP からカードのみ追加取り込み（デスクトップ・Android 共通）
  // readFile() で bytes を取得することで Android の content:// URI にも対応する (§10.2)。
  const handleImportCardsZip = async () => {
    const file = await open({
      multiple: false,
      title: "カードZIPを選択",
      filters: [{ name: "カード (zip)", extensions: ["zip"] }],
    });
    if (!file || typeof file !== "string") return;
    try {
      const bytes = await readFile(file);
      const res: ImportResult = await call(commands.importCardsZipBytes(deckId, Array.from(bytes)));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      deck.reload();
      progress.reload();
    } catch (e) {
      setImportMsg(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async () => {
    try {
      await call(commands.deleteDeck(deckId));
      navigate({ to: "/decks" });
    } catch (e) {
      alert(e instanceof Error ? e.message : String(e));
    }
  };

  if (deck.loading) return <Loading />;
  if (deck.error) return <ErrorBox message={deck.error} />;
  if (!deck.data) return null;
  const d = deck.data;

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <Link to="/decks" className="text-sm text-muted-foreground hover:underline">
            ← デッキ一覧
          </Link>
          <h1 className="mt-1 text-2xl font-bold">{d.name}</h1>
          {d.description && <p className="mt-1 text-muted-foreground">{d.description}</p>}
        </div>
        <Button variant="ghost" size="icon" onClick={() => setConfirmDelete(true)} title="デッキを削除">
          <Trash2 className="h-5 w-5 text-destructive" />
        </Button>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          size="lg"
          onClick={() => navigate({ to: "/decks/$deckId/study", params: { deckId } })}
        >
          <PlayCircle className="h-5 w-5" />
          学習開始
        </Button>
        <Button
          variant="outline"
          onClick={() => navigate({ to: "/decks/$deckId/cards", params: { deckId } })}
        >
          <List className="h-4 w-4" />
          カード一覧（{d.card_count}）
        </Button>
        <Button variant="outline" onClick={handleImportCardsZip}>
          <FileArchive className="h-4 w-4" />
          ZIPでカード追加
        </Button>
        {/* フォルダ取り込みは Android の SAF では機能しないため非表示 (§10.2.1) */}
        {!isAndroid() && (
          <Button variant="outline" onClick={handleImportCards}>
            <FolderInput className="h-4 w-4" />
            フォルダでカード追加
          </Button>
        )}
      </div>

      {importMsg && (
        <div className="rounded-md border bg-accent/40 px-4 py-2 text-sm">{importMsg}</div>
      )}

      {progress.data && <StudyProgress deck={d} progress={progress.data} />}

      <Card>
        <CardHeader className="flex-row items-center justify-between space-y-0">
          <CardTitle className="text-base">デッキ設定</CardTitle>
          {!editingSettings && (
            <Button
              variant="ghost"
              size="sm"
              className="gap-1.5"
              onClick={() => setEditingSettings(true)}
            >
              <Pencil className="h-4 w-4" />
              編集
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {editingSettings ? (
            <DeckSettingsForm
              deck={d}
              onCancel={() => setEditingSettings(false)}
              onSaved={() => {
                setEditingSettings(false);
                deck.reload();
                progress.reload();
              }}
            />
          ) : (
            <div className="space-y-3 text-sm">
              <Row label="言語">{d.language}</Row>
              <Row label="テストモード">
                <div className="flex gap-1.5">
                  {d.test_modes.map((m) => (
                    <Badge key={m} variant="secondary">
                      {modeLabel(m)}
                    </Badge>
                  ))}
                </div>
              </Row>
              <Row label="1日の新規上限">{d.daily_new_limit} 枚</Row>
              <Row label="1日の復習上限">{d.daily_review_limit} 枚</Row>
              <Row label="1日の学習量の目安">
                {d.daily_study_target != null ? `${d.daily_study_target} 枚` : "無効"}
                {d.new_limited_by_study && (
                  <span className="ml-2 text-xs font-normal text-amber-600">
                    復習が多いため新規を調整中
                  </span>
                )}
              </Row>
              <Row label="目標定着率">{(d.fsrs_target_retention * 100).toFixed(0)}%</Row>
              <Row label="最大復習間隔">{d.fsrs_max_interval_days} 日</Row>
            </div>
          )}
        </CardContent>
      </Card>

      <ConfirmDialog
        open={confirmDelete}
        title="デッキを削除"
        message={`「${d.name}」を削除します。\nカード・学習履歴もすべて削除され、元に戻せません。`}
        confirmLabel="削除する"
        destructive
        onConfirm={handleDelete}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between border-b pb-2 last:border-0 last:pb-0">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium">{children}</span>
    </div>
  );
}

/** デッキ全体の習得度を、積み上げバー＋凡例＋モード別バーで表示する。 */
function StudyProgress({ deck, progress }: { deck: Deck; progress: DeckProgress }) {
  // カード0枚など学習単位が無いデッキでは表示しない
  if (progress.total_units === 0) return null;

  const { new_count, learning_count, young_count, mature_count, total_units, modes, completed_today } = progress;
  // 習得率 = review フェーズ（習得中＋定着）に到達したユニットの割合
  const learnedPct = Math.round(((young_count + mature_count) / total_units) * 100);
  const dueToday = deck.new_today + deck.review_today;
  // モードが複数あるときだけモード別の内訳を出す（単一モードでは全体バーと同じため）
  const showModes = modes.length >= 2;

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">学習進捗</CardTitle>
        <div className="text-sm text-muted-foreground">
          習得率 <span className="text-lg font-bold text-foreground">{learnedPct}%</span>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <SegBar
          className="h-3"
          total={total_units}
          mature={mature_count}
          young={young_count}
          learning={learning_count}
        />
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <LegendDot className="bg-green-600" label="定着" count={mature_count} />
          <LegendDot className="bg-green-400" label="習得中" count={young_count} />
          <LegendDot className="bg-amber-400" label="学習中" count={learning_count} />
          <LegendDot className="bg-slate-300 dark:bg-slate-600" label="未学習" count={new_count} />
        </div>

        {showModes && (
          <div className="space-y-2 border-t pt-3">
            {modes.map((m) => {
              const mTotal = m.new_count + m.learning_count + m.young_count + m.mature_count;
              const mPct =
                mTotal > 0 ? Math.round(((m.young_count + m.mature_count) / mTotal) * 100) : 0;
              return (
                <div key={m.mode} className="flex items-center gap-3 text-xs">
                  <span className="w-16 shrink-0 truncate text-muted-foreground">
                    {modeLabel(m.mode)}
                  </span>
                  <SegBar
                    className="h-1.5 flex-1 min-w-0"
                    total={mTotal}
                    mature={m.mature_count}
                    young={m.young_count}
                    learning={m.learning_count}
                  />
                  <span className="w-9 shrink-0 text-right tabular-nums text-muted-foreground">
                    {mPct}%
                  </span>
                </div>
              );
            })}
          </div>
        )}

        <div className="border-t pt-3 text-sm text-muted-foreground">
          今日：完了 {completed_today} / 予定 {dueToday}
          {deck.learning_today > 0 && (
            <span className="ml-1 text-xs">・学習中 {deck.learning_today}</span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

/** 定着→習得中→学習中の順に左から積み上げる進捗バー。残り（未学習）はトラック色で表現する。 */
function SegBar({
  total,
  mature,
  young,
  learning,
  className,
}: {
  total: number;
  mature: number;
  young: number;
  learning: number;
  className?: string;
}) {
  const seg = (n: number, cls: string) =>
    n > 0 ? (
      <div className={cls} style={{ width: `${(n / total) * 100}%`, minWidth: 2 }} />
    ) : null;
  return (
    <div className={cn("flex overflow-hidden rounded-full bg-slate-200 dark:bg-slate-700", className)}>
      {seg(mature, "bg-green-600")}
      {seg(young, "bg-green-400")}
      {seg(learning, "bg-amber-400")}
    </div>
  );
}

function LegendDot({ className, label, count }: { className: string; label: string; count: number }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className={cn("inline-block h-2 w-2 rounded-sm", className)} />
      {label} {count}
    </span>
  );
}
