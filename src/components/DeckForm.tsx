import { useState } from "react";
import type { CreateDeckInput } from "@/bindings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { modeLabel } from "@/components/common";

export const AVAILABLE_MODES = ["recognition", "pronunciation", "production"] as const;

interface Props {
  onSubmit: (input: CreateDeckInput) => Promise<void> | void;
  onCancel: () => void;
  submitting?: boolean;
}

/** 新規デッキ作成フォーム (spec §8.1) */
export function DeckForm({ onSubmit, onCancel, submitting }: Props) {
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [language, setLanguage] = useState("zh");
  const [testModes, setTestModes] = useState<string[]>(["recognition", "pronunciation"]);
  const [dailyNew, setDailyNew] = useState(20);
  const [dailyReview, setDailyReview] = useState(100);
  const [retention, setRetention] = useState(0.9);
  const [maxInterval, setMaxInterval] = useState(365);
  const [error, setError] = useState<string | null>(null);

  const toggleMode = (m: string) =>
    setTestModes((cur) => (cur.includes(m) ? cur.filter((x) => x !== m) : [...cur, m]));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!/^[A-Za-z0-9_]+$/.test(id)) {
      setError("デッキIDは英数字とアンダースコアのみ使用できます。");
      return;
    }
    if (!name.trim()) {
      setError("デッキ名を入力してください。");
      return;
    }
    if (testModes.length === 0) {
      setError("テストモードを1つ以上選択してください。");
      return;
    }
    await onSubmit({
      id,
      name,
      description: description.trim() === "" ? null : description,
      language,
      test_modes: testModes,
      daily_new_limit: dailyNew,
      daily_review_limit: dailyReview,
      fsrs_target_retention: retention,
      fsrs_max_interval_days: maxInterval,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {error && <div className="text-sm text-destructive">{error}</div>}
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1">
          <Label htmlFor="deck-id">デッキID（英数字・_）</Label>
          <Input id="deck-id" value={id} onChange={(e) => setId(e.target.value)} placeholder="hsk3" />
        </div>
        <div className="space-y-1">
          <Label htmlFor="deck-lang">言語</Label>
          <Input id="deck-lang" value={language} onChange={(e) => setLanguage(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label htmlFor="deck-name">デッキ名</Label>
        <Input id="deck-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="HSK3 単語" />
      </div>
      <div className="space-y-1">
        <Label htmlFor="deck-desc">説明（任意）</Label>
        <Textarea id="deck-desc" value={description} onChange={(e) => setDescription(e.target.value)} />
      </div>
      <div className="space-y-2">
        <Label>テストモード（複数選択可）</Label>
        <div className="flex flex-wrap gap-3">
          {AVAILABLE_MODES.map((m) => (
            <label key={m} className="flex cursor-pointer items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={testModes.includes(m)}
                onChange={() => toggleMode(m)}
                className="h-4 w-4"
              />
              {modeLabel(m)}（{m}）
            </label>
          ))}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1">
          <Label htmlFor="deck-new">1日の新規上限</Label>
          <Input id="deck-new" type="number" value={dailyNew} onChange={(e) => setDailyNew(Number(e.target.value))} />
        </div>
        <div className="space-y-1">
          <Label htmlFor="deck-review">1日の復習上限</Label>
          <Input id="deck-review" type="number" value={dailyReview} onChange={(e) => setDailyReview(Number(e.target.value))} />
        </div>
        <div className="space-y-1">
          <Label htmlFor="deck-ret">目標定着率 (0-1)</Label>
          <Input id="deck-ret" type="number" step="0.01" min="0.7" max="0.99" value={retention} onChange={(e) => setRetention(Number(e.target.value))} />
        </div>
        <div className="space-y-1">
          <Label htmlFor="deck-max">最大復習間隔（日）</Label>
          <Input id="deck-max" type="number" value={maxInterval} onChange={(e) => setMaxInterval(Number(e.target.value))} />
        </div>
      </div>
      <div className="flex justify-end gap-2 pt-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          キャンセル
        </Button>
        <Button type="submit" disabled={submitting}>
          {submitting ? "作成中…" : "作成"}
        </Button>
      </div>
    </form>
  );
}
