import { useEffect, useState } from "react";
import { Link, useParams } from "@tanstack/react-router";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { Loading, ErrorBox } from "@/components/common";

export function CardDetailPage() {
  const { deckId, cardId } = useParams({ strict: false }) as {
    deckId: string;
    cardId: string;
  };
  const card = useAsync(() => call(commands.getCard(cardId, deckId)), [cardId, deckId]);

  const [notes, setNotes] = useState("");
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (card.data) setNotes(card.data.user_notes);
  }, [card.data]);

  const saveNotes = async () => {
    await call(commands.updateUserNotes(cardId, deckId, notes));
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  if (card.loading) return <Loading />;
  if (card.error) return <ErrorBox message={card.error} />;
  if (!card.data) return null;
  const c = card.data;

  return (
    <div className="mx-auto max-w-2xl space-y-5">
      <Link to="/decks/$deckId/cards" params={{ deckId }} className="text-sm text-muted-foreground hover:underline">
        ← カード一覧
      </Link>

      <div className="rounded-lg border bg-card p-6 text-center md:p-8">
        <div className="hanzi text-5xl font-bold">{c.hanzi}</div>
        <div className="mt-2 text-muted-foreground">{c.pinyin_accepted.join(" / ")}</div>
        <div className="mt-2 text-lg">{c.meaning}</div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">詳細</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm">
          {c.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {c.tags.map((t) => (
                <Badge key={t} variant="secondary">
                  {t}
                </Badge>
              ))}
            </div>
          )}
          {c.example_sentences.length > 0 && (
            <div>
              <div className="mb-1 text-muted-foreground">例文</div>
              <div className="space-y-2">
                {c.example_sentences.map((ex, i) => (
                  <div key={i} className="rounded bg-accent/40 p-2">
                    <div className="hanzi">{ex.text}</div>
                    {ex.pinyin && <div className="text-muted-foreground">{ex.pinyin}</div>}
                    {ex.translation && <div>{ex.translation}</div>}
                  </div>
                ))}
              </div>
            </div>
          )}
          {c.synonyms.length > 0 && <Field label="類義語">{c.synonyms.join("、")}</Field>}
          {c.antonyms.length > 0 && <Field label="対義語">{c.antonyms.join("、")}</Field>}
          {c.ai_notes && <Field label="AIメモ">{c.ai_notes}</Field>}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">ユーザーメモ</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="メモを入力…" />
          <div className="flex items-center gap-3">
            <Button onClick={saveNotes}>保存</Button>
            {saved && <span className="text-sm text-green-600">保存しました</span>}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex gap-2">
      <span className="shrink-0 text-muted-foreground">{label}：</span>
      <span>{children}</span>
    </div>
  );
}
