import { useState, type ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { FileArchive, FolderInput, Plus } from "lucide-react";
import type { CreateDeckInput, ImportResult } from "@/bindings";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { isAndroid } from "@/lib/platform";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Modal } from "@/components/ui/modal";
import { DeckForm } from "@/components/DeckForm";
import { Loading, ErrorBox } from "@/components/common";

export function DecksPage() {
  const decks = useAsync(() => call(commands.getDecks()), []);
  const [showForm, setShowForm] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [importMsg, setImportMsg] = useState<string | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);

  const handleCreate = async (input: CreateDeckInput) => {
    setSubmitting(true);
    try {
      await call(commands.createDeck(input));
      setShowForm(false);
      decks.reload();
    } catch (e) {
      alert(e instanceof Error ? e.message : String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const handleImportDeck = async () => {
    const folder = await open({ directory: true, title: "デッキフォルダを選択" });
    if (!folder || typeof folder !== "string") return;
    try {
      const res: ImportResult = await call(commands.importDeckFolder(folder));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      decks.reload();
    } catch (e) {
      setImportMsg(e instanceof Error ? e.message : String(e));
    }
  };

  // ZIP 取り込み（デスクトップ・Android 共通）。deck.json を含む zip を選択する。
  // readFile() で bytes を取得することで Android の content:// URI にも対応する (§10.2)。
  const handleImportDeckZip = async () => {
    const file = await open({
      multiple: false,
      title: "デッキZIPを選択",
      filters: [{ name: "デッキ (zip)", extensions: ["zip"] }],
    });
    if (!file || typeof file !== "string") return;
    try {
      const bytes = await readFile(file);
      const res: ImportResult = await call(commands.importDeckZipBytes(Array.from(bytes)));
      setImportMsg(`インポート完了：新規 ${res.created} 件、更新 ${res.updated} 件`);
      decks.reload();
    } catch (e) {
      setImportMsg(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-2xl font-bold">デッキ一覧</h1>

        {/* デスクトップ (md+): 従来どおりボタンを横並び（Windows 版は不変） */}
        <div className="hidden gap-2 md:flex">
          <Button variant="outline" onClick={handleImportDeckZip}>
            <FileArchive className="h-4 w-4" />
            ZIPから取り込み
          </Button>
          {/* フォルダ取り込みは Android の SAF では機能しないため非表示 (§10.2.1) */}
          {!isAndroid() && (
            <Button variant="outline" onClick={handleImportDeck}>
              <FolderInput className="h-4 w-4" />
              フォルダから取り込み
            </Button>
          )}
          <Button onClick={() => setShowForm(true)}>
            <Plus className="h-4 w-4" />
            新規デッキ作成
          </Button>
        </div>

        {/* モバイル (< md): アクションを「＋」メニューに集約 */}
        <div className="relative md:hidden">
          <Button
            size="icon"
            aria-label="操作メニュー"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((v) => !v)}
          >
            <Plus className="h-5 w-5" />
          </Button>
          {menuOpen && (
            <>
              {/* 画面外タップで閉じるためのオーバーレイ */}
              <div
                className="fixed inset-0 z-30"
                onClick={() => setMenuOpen(false)}
              />
              <div
                role="menu"
                className="absolute right-0 z-40 mt-2 w-52 overflow-hidden rounded-md border bg-card py-1 shadow-lg"
              >
                <MenuItem
                  onClick={() => {
                    setMenuOpen(false);
                    setShowForm(true);
                  }}
                >
                  <Plus className="h-4 w-4" />
                  新規デッキ作成
                </MenuItem>
                <MenuItem
                  onClick={() => {
                    setMenuOpen(false);
                    void handleImportDeckZip();
                  }}
                >
                  <FileArchive className="h-4 w-4" />
                  ZIPから取り込み
                </MenuItem>
                {!isAndroid() && (
                  <MenuItem
                    onClick={() => {
                      setMenuOpen(false);
                      void handleImportDeck();
                    }}
                  >
                    <FolderInput className="h-4 w-4" />
                    フォルダから取り込み
                  </MenuItem>
                )}
              </div>
            </>
          )}
        </div>
      </div>

      {importMsg && (
        <div className="rounded-md border bg-accent/40 px-4 py-2 text-sm">{importMsg}</div>
      )}

      {decks.loading ? (
        <Loading />
      ) : decks.error ? (
        <ErrorBox message={decks.error} />
      ) : decks.data && decks.data.length > 0 ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {decks.data.map((d) => (
            <Link key={d.id} to="/decks/$deckId" params={{ deckId: d.id }}>
              <Card className="transition-colors hover:border-primary">
                <CardContent className="p-5">
                  <div className="text-lg font-semibold">{d.name}</div>
                  {d.description && (
                    <div className="mt-1 line-clamp-2 text-sm text-muted-foreground">
                      {d.description}
                    </div>
                  )}
                  <div className="mt-3 flex gap-4 text-sm text-muted-foreground">
                    <span>カード {d.card_count} 枚</span>
                    <span>今日の復習 {d.due_today} 枚</span>
                  </div>
                </CardContent>
              </Card>
            </Link>
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-dashed p-10 text-center text-muted-foreground">
          デッキがまだありません。「新規デッキ作成」または「デッキをまるごとインポート」から始めましょう。
        </div>
      )}

      <Modal open={showForm} onClose={() => setShowForm(false)} title="新規デッキ作成">
        <DeckForm
          onSubmit={handleCreate}
          onCancel={() => setShowForm(false)}
          submitting={submitting}
        />
      </Modal>
    </div>
  );
}

function MenuItem({
  onClick,
  children,
}: {
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className="flex w-full items-center gap-2 px-4 py-2.5 text-left text-sm hover:bg-accent"
    >
      {children}
    </button>
  );
}
