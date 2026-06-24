import { useMemo, useState } from "react";
import { Link, useParams } from "@tanstack/react-router";
import type { CardFilter } from "@/bindings";
import { call, commands } from "@/lib/api";
import { useAsync } from "@/lib/useAsync";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Loading, ErrorBox, StateBadge } from "@/components/common";
import { cn } from "@/lib/utils";

const STATE_OPTIONS = [
  { value: "", label: "すべての状態" },
  { value: "new", label: "New" },
  { value: "learning", label: "Learning" },
  { value: "review", label: "Review" },
  { value: "relearning", label: "Relearning" },
];

export function CardsPage() {
  const { deckId } = useParams({ strict: false }) as { deckId: string };
  const [search, setSearch] = useState("");
  const [state, setState] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);

  // 全カード（タグ候補の収集用）
  const allCards = useAsync(() => call(commands.getCards(deckId, null)), [deckId]);
  const allTags = useMemo(() => {
    const set = new Set<string>();
    allCards.data?.forEach((c) => c.tags.forEach((t) => set.add(t)));
    return Array.from(set).sort();
  }, [allCards.data]);

  const filter: CardFilter = {
    search_text: search.trim() === "" ? null : search,
    tags: selectedTags.length > 0 ? selectedTags : null,
    srs_state: state === "" ? null : state,
  };
  const cards = useAsync(
    () => call(commands.getCards(deckId, filter)),
    [deckId, search, state, selectedTags.join(",")],
  );

  const toggleTag = (t: string) =>
    setSelectedTags((cur) => (cur.includes(t) ? cur.filter((x) => x !== t) : [...cur, t]));

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <Link to="/decks/$deckId" params={{ deckId }} className="text-sm text-muted-foreground hover:underline">
          ← デッキ詳細
        </Link>
      </div>
      <h1 className="text-2xl font-bold">カード一覧</h1>

      <div className="flex flex-wrap items-center gap-3">
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="漢字・意味で検索…"
          className="w-full sm:max-w-xs"
        />
        <select
          value={state}
          onChange={(e) => setState(e.target.value)}
          className="h-10 rounded-md border border-input bg-background px-3 text-sm"
        >
          {STATE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>

      {allTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {allTags.map((t) => (
            <button key={t} onClick={() => toggleTag(t)}>
              <Badge
                variant={selectedTags.includes(t) ? "default" : "outline"}
                className={cn("cursor-pointer")}
              >
                {t}
              </Badge>
            </button>
          ))}
        </div>
      )}

      {cards.loading ? (
        <Loading />
      ) : cards.error ? (
        <ErrorBox message={cards.error} />
      ) : cards.data && cards.data.length > 0 ? (
        <div className="space-y-2">
          {cards.data.map((c) => (
            <Link key={c.id} to="/decks/$deckId/cards/$cardId" params={{ deckId, cardId: c.id }}>
              <Card className="transition-colors hover:border-primary">
                <CardContent className="flex items-center justify-between gap-3 p-4">
                  <div className="flex min-w-0 items-center gap-3">
                    <span className="hanzi shrink-0 text-xl font-semibold">{c.hanzi}</span>
                    <span className="shrink-0 text-sm text-muted-foreground">
                      {c.pinyin_accepted[0] ?? ""}
                    </span>
                    <span className="truncate text-sm">{c.meaning}</span>
                  </div>
                  <div className="flex shrink-0 flex-wrap justify-end gap-1">
                    {c.srs_states.map((ms) => (
                      <StateBadge key={ms.mode} state={ms.state} />
                    ))}
                  </div>
                </CardContent>
              </Card>
            </Link>
          ))}
        </div>
      ) : (
        <div className="rounded-md border border-dashed p-10 text-center text-muted-foreground">
          該当するカードがありません。
        </div>
      )}
    </div>
  );
}
