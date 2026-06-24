import { useState } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, FolderInput, List, PlayCircle, Trash2 } from "lucide-react";
import type { ImportResult } from "@/bindings";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ConfirmDialog } from "@/components/ui/modal";
import { Loading, ErrorBox, modeLabel } from "@/components/common";

export function DeckDetailPage() {
  const { deckId } = useParams({ strict: false }) as { deckId: string };
  const navigate = useNavigate();
  const deck = useAsync(() => call(commands.getDeck(deckId)), [deckId]);
  const [importMsg, setImportMsg] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const handleImportCards = async () => {
    const folder = await open({ directory: true, title: "カードフォルダを選択" });
    if (!folder || typeof folder !== "string") return;
    try {
      const res: ImportResult = await call(commands.importCardsFolder(deckId, folder));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      deck.reload();
    } catch (e) {
      setImportMsg(e instanceof Error ? e.message : String(e));
    }
  };

  // ZIP からカードのみ追加取り込み（デスクトップ・Android 共通）
  const handleImportCardsZip = async () => {
    const file = await open({
      multiple: false,
      title: "カードZIPを選択",
      filters: [{ name: "カード (zip)", extensions: ["zip"] }],
    });
    if (!file || typeof file !== "string") return;
    try {
      const res: ImportResult = await call(commands.importCardsZip(deckId, file));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      deck.reload();
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
        <Button variant="outline" onClick={handleImportCards}>
          <FolderInput className="h-4 w-4" />
          フォルダでカード追加
        </Button>
      </div>

      {importMsg && (
        <div className="rounded-md border bg-accent/40 px-4 py-2 text-sm">{importMsg}</div>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">デッキ設定</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm">
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
          <Row label="目標定着率">{(d.fsrs_target_retention * 100).toFixed(0)}%</Row>
          <Row label="最大復習間隔">{d.fsrs_max_interval_days} 日</Row>
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
