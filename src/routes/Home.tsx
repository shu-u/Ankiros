import { Link, useNavigate } from "@tanstack/react-router";
import { Flame, PlayCircle } from "lucide-react";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { useAppStore } from "@/store/appStore";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Loading, ErrorBox } from "@/components/common";

export function HomePage() {
  const navigate = useNavigate();
  const lastUsedDeckId = useAppStore((s) => s.lastUsedDeckId);
  const stats = useAsync(() => call(commands.getHomeStats()), []);

  if (stats.loading) return <Loading />;
  if (stats.error) return <ErrorBox message={stats.error} />;
  if (!stats.data) return null;

  const { streak_days, today_reviewed, deck_due_counts, seven_day_forecast } = stats.data;
  const lastDeck = deck_due_counts.find((d) => d.deck_id === lastUsedDeckId);
  const maxForecast = Math.max(1, ...seven_day_forecast.map((d) => d.count));

  return (
    <div className="space-y-8">
      <h1 className="text-2xl font-bold">ホーム</h1>

      {/* 今すぐ学習 */}
      <Card>
        <CardContent className="flex items-center justify-between p-6">
          {lastUsedDeckId && lastDeck ? (
            <>
              <div>
                <div className="text-sm text-muted-foreground">最後に使ったデッキ</div>
                <div className="text-xl font-semibold">{lastDeck.deck_name}</div>
                <div className="mt-1 text-sm text-muted-foreground">
                  今日の予定 {lastDeck.due_count} 枚 ・ 完了 {lastDeck.completed_today} 枚
                </div>
              </div>
              <Button
                size="lg"
                onClick={() =>
                  navigate({ to: "/decks/$deckId/study", params: { deckId: lastUsedDeckId } })
                }
              >
                <PlayCircle className="h-5 w-5" />
                今すぐ学習
              </Button>
            </>
          ) : (
            <div className="text-muted-foreground">
              まずは
              <Link to="/decks" className="mx-1 text-primary underline">
                デッキ一覧
              </Link>
              からデッキを選んで学習を始めましょう。
            </div>
          )}
        </CardContent>
      </Card>

      {/* 統計 */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Flame className="h-4 w-4 text-orange-500" />
              連続学習日数
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-4xl font-bold">
              {streak_days}
              <span className="ml-1 text-base font-normal text-muted-foreground">日</span>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-base">今日の完了数</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-4xl font-bold">
              {today_reviewed}
              <span className="ml-1 text-base font-normal text-muted-foreground">枚</span>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* デッキ別 完了/予定 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">デッキ別 今日の進捗</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          {deck_due_counts.length === 0 ? (
            <div className="text-sm text-muted-foreground">デッキがありません。</div>
          ) : (
            deck_due_counts.map((d) => (
              <div
                key={d.deck_id}
                className="flex items-center justify-between rounded-md border px-3 py-2 text-sm"
              >
                <span className="font-medium">{d.deck_name}</span>
                <span className="text-muted-foreground">
                  完了 {d.completed_today} / 予定 {d.due_count}
                </span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* 7日間の予定枚数 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">7日間の予定枚数</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-end gap-2" style={{ height: 140 }}>
            {seven_day_forecast.map((d, i) => (
              <div key={d.date} className="flex flex-1 flex-col items-center gap-1">
                <div className="text-xs text-muted-foreground">{d.count}</div>
                <div
                  className={`w-full rounded-t ${d.is_past ? "bg-muted-foreground/40" : "bg-primary/80"}`}
                  style={{ height: `${(d.count / maxForecast) * 100}%`, minHeight: 2 }}
                />
                <div className="text-xs text-muted-foreground">
                  {d.is_past ? "昨日" : i === 1 ? "今日" : d.date.slice(5)}
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
