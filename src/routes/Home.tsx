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
  const forecast = seven_day_forecast.map((d) => ({
    ...d,
    total: d.reviews + d.new_cards + d.overdue,
  }));
  const maxForecast = Math.max(1, ...forecast.map((d) => d.total));
  const hasOverdue = forecast.some((d) => d.overdue > 0);

  return (
    <div className="space-y-8">
      <h1 className="text-2xl font-bold">ホーム</h1>

      {/* 今すぐ学習 */}
      <Card>
        <CardContent className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between">
          {lastUsedDeckId && lastDeck ? (
            <>
              <div className="min-w-0">
                <div className="text-sm text-muted-foreground">最後に使ったデッキ</div>
                <div className="truncate text-xl font-semibold">{lastDeck.deck_name}</div>
                <div className="mt-1 text-sm text-muted-foreground">
                  今日の予定 {lastDeck.new_count + lastDeck.review_count} 枚
                  <span className="text-xs">（新規 {lastDeck.new_count}・復習 {lastDeck.review_count}）</span>
                  ・ 完了 {lastDeck.completed_today} 枚
                </div>
                {lastDeck.learning_count > 0 && (
                  <div className="text-xs text-muted-foreground">
                    学習中 {lastDeck.learning_count} 枚（同日中に再出題）
                  </div>
                )}
              </div>
              <Button
                size="lg"
                className="w-full sm:w-auto sm:shrink-0"
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
                className="flex items-center justify-between gap-3 rounded-md border px-3 py-2 text-sm"
              >
                <span className="min-w-0 truncate font-medium">{d.deck_name}</span>
                <span className="shrink-0 whitespace-nowrap text-right text-muted-foreground">
                  完了 {d.completed_today} / 予定 {d.new_count + d.review_count}
                  {d.learning_count > 0 && (
                    <span className="ml-1 text-xs">・学習中 {d.learning_count}</span>
                  )}
                </span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* 今後7日間の予定（先読み負荷）*/}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">今後7日間の予定</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2" style={{ height: 160 }}>
            {forecast.map((d, i) => (
              <div key={d.date} className="flex flex-1 flex-col items-center gap-1">
                <div className="text-xs tabular-nums text-muted-foreground">{d.total}</div>
                {/* バー描画領域: flex-1 で確定高さを持たせ、% 指定の各セグメントを解決させる */}
                <div className="flex w-full flex-1 flex-col-reverse overflow-hidden rounded-t">
                  {d.overdue > 0 && (
                    <div
                      className="w-full shrink-0 bg-amber-500/80"
                      style={{ height: `${(d.overdue / maxForecast) * 100}%`, minHeight: 2 }}
                      title={`延滞 ${d.overdue}`}
                    />
                  )}
                  {d.reviews > 0 && (
                    <div
                      className="w-full shrink-0 bg-primary/80"
                      style={{ height: `${(d.reviews / maxForecast) * 100}%`, minHeight: 2 }}
                      title={`復習 ${d.reviews}`}
                    />
                  )}
                  {d.new_cards > 0 && (
                    <div
                      className="w-full shrink-0 bg-emerald-500/80"
                      style={{ height: `${(d.new_cards / maxForecast) * 100}%`, minHeight: 2 }}
                      title={`新規 ${d.new_cards}`}
                    />
                  )}
                </div>
                <div className="whitespace-nowrap text-xs text-muted-foreground">
                  {i === 0 ? "今日" : d.date.slice(5)}
                </div>
              </div>
            ))}
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
            <span className="flex items-center gap-1.5">
              <span className="inline-block h-2 w-2 rounded-sm bg-primary/80" />復習
            </span>
            <span className="flex items-center gap-1.5">
              <span className="inline-block h-2 w-2 rounded-sm bg-emerald-500/80" />新規
            </span>
            {hasOverdue && (
              <span className="flex items-center gap-1.5">
                <span className="inline-block h-2 w-2 rounded-sm bg-amber-500/80" />延滞
              </span>
            )}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            ※ 先の予定は現時点の下限です（復習後に再設定される期日は未反映）
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
