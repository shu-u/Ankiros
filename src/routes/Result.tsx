import { useEffect } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { useSessionStore } from "@/store/sessionStore";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function ResultPage() {
  const { deckId } = useParams({ strict: false }) as { deckId: string };
  const navigate = useNavigate();
  const { isComplete, deckId: storeDeckId, results } = useSessionStore();

  // ページリロード等で store が消えた場合はデッキ詳細へリダイレクト (spec §11)
  useEffect(() => {
    if (!isComplete || storeDeckId === null) {
      navigate({ to: "/decks/$deckId", params: { deckId } });
    }
  }, [isComplete, storeDeckId, deckId, navigate]);

  const tomorrow = useAsync(() => call(commands.getHomeStats()), []);

  if (!isComplete || storeDeckId === null) return null;

  const againCount =
    results.total - results.hardCount - results.goodCount - results.easyCount;
  const tomorrowCount = tomorrow.data?.seven_day_forecast?.[1]?.count ?? null;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-center text-2xl font-bold">セッション完了！</h1>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">復習カード：{results.total} 枚</CardTitle>
        </CardHeader>
        <CardContent className="space-y-1 text-sm">
          <CountRow label="Again" count={againCount} color="text-red-600" />
          <CountRow label="Hard" count={results.hardCount} color="text-amber-600" />
          <CountRow label="Good" count={results.goodCount} color="text-green-600" />
          <CountRow label="Easy" count={results.easyCount} color="text-blue-600" />
        </CardContent>
      </Card>

      {tomorrowCount !== null && (
        <div className="rounded-md border bg-accent/40 px-4 py-3 text-sm">
          明日の予定：{tomorrowCount} 枚
        </div>
      )}

      {results.againCards.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">もう一度確認しましょう</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {results.againCards.map((c) => (
              <div
                key={`${c.card.id}_${c.mode}`}
                className="flex items-center justify-between rounded-md border px-3 py-2 text-sm"
              >
                <span className="flex items-center gap-3">
                  <span className="hanzi font-medium">{c.card.hanzi}</span>
                  <span className="text-muted-foreground">
                    {c.card.pinyin_accepted[0] ?? ""}
                  </span>
                </span>
                <Link
                  to="/decks/$deckId/cards/$cardId"
                  params={{ deckId, cardId: c.card.id }}
                  className="text-primary hover:underline"
                >
                  詳細を見る
                </Link>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      <div className="flex justify-center">
        <Button size="lg" onClick={() => navigate({ to: "/" })}>
          ホームへ戻る
        </Button>
      </div>
    </div>
  );
}

function CountRow({ label, count, color }: { label: string; count: number; color: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className={`font-medium ${color}`}>{label}</span>
      <span>{count} 枚</span>
    </div>
  );
}
