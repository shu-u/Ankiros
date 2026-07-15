import { useState } from "react";
import type { Deck, UpdateDeckInput } from "@/bindings";
import { call, commands } from "@/lib/api";
import { logger } from "@/lib/logger";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { ConfirmDialog } from "@/components/ui/modal";
import { AVAILABLE_MODES } from "@/components/DeckForm";
import { modeLabel } from "@/components/common";

interface Props {
  deck: Deck;
  /** 保存成功時（呼び出し側で reload する） */
  onSaved: () => void;
  onCancel: () => void;
}

/**
 * デッキ設定のアプリ内編集フォーム（DeckDetail のデッキ設定カード内に表示）。
 *
 * 学習進捗DB（srs_records）を壊さない範囲のみ編集可能にしている:
 * - デッキID・言語は編集不可（IDは外部キー、言語は挙動未使用）。
 * - テストモードは「追加のみ」。既存モードの削除は進捗の不整合を招くためロックする。
 * - 名前/説明/上限/FSRS係数は decks 行のみの更新で、srs_records に影響しない。
 *
 * レイアウトは Android で横幅が広がりすぎないよう1カラム基調。数値項目のみ sm 以上で2カラム。
 */
export function DeckSettingsForm({ deck, onSaved, onCancel }: Props) {
  const [name, setName] = useState(deck.name);
  const [description, setDescription] = useState(deck.description ?? "");
  const [testModes, setTestModes] = useState<string[]>(deck.test_modes);
  const [dailyNew, setDailyNew] = useState(deck.daily_new_limit);
  const [dailyReview, setDailyReview] = useState(deck.daily_review_limit);
  // 1日の学習量の目安（null なら無効）。有効時の初期値は現在値、無ければ復習上限を仮置き。
  const [studyTargetOn, setStudyTargetOn] = useState(deck.daily_study_target != null);
  const [studyTarget, setStudyTarget] = useState(deck.daily_study_target ?? deck.daily_review_limit);
  const [retention, setRetention] = useState(deck.fsrs_target_retention);
  const [maxInterval, setMaxInterval] = useState(deck.fsrs_max_interval_days);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);

  // 既存モードは削除不可（ロック）。追加のみ許可する。
  const original = deck.test_modes;
  // 追加されるモード。保存後は削除できない不可逆な変更なので、確認ダイアログで明示する。
  const addedModes = testModes.filter((m) => !original.includes(m));
  const confirmMessage = buildConfirmMessage(addedModes.map(modeLabel));

  const toggleMode = (m: string, checked: boolean) =>
    setTestModes((cur) => (checked ? [...cur, m] : cur.filter((x) => x !== m)));

  const validate = (): boolean => {
    if (!name.trim()) {
      setError("デッキ名を入力してください。");
      return false;
    }
    if (!(retention >= 0.7 && retention <= 0.99)) {
      setError("目標定着率は 0.70〜0.99 の範囲で入力してください。");
      return false;
    }
    if (dailyNew < 0 || dailyReview < 0) {
      setError("1日の上限は0以上で入力してください。");
      return false;
    }
    if (studyTargetOn && studyTarget < 0) {
      setError("1日の学習量の目安は0以上で入力してください。");
      return false;
    }
    if (maxInterval < 1) {
      setError("最大復習間隔は1日以上で入力してください。");
      return false;
    }
    setError(null);
    return true;
  };

  // 保存ボタン：まず入力を検証し、問題なければ確認ダイアログを開く。
  const handleSaveClick = () => {
    if (validate()) setConfirmOpen(true);
  };

  // 確認後の実保存。decks 行のみ更新し、srs_records には影響しない。
  const handleConfirmedSave = async () => {
    setConfirmOpen(false);
    setSaving(true);
    try {
      const input: UpdateDeckInput = {
        name: name.trim(),
        description: description.trim() === "" ? null : description,
        language: deck.language, // 言語は編集対象外：現在値を維持する
        test_modes: testModes,
        daily_new_limit: dailyNew,
        daily_review_limit: dailyReview,
        daily_study_target: studyTargetOn ? studyTarget : null,
        fsrs_target_retention: retention,
        fsrs_max_interval_days: maxInterval,
      };
      await call(commands.updateDeck(deck.id, input));
      void logger.info(`Deck settings updated: ${deck.id}`);
      onSaved();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-4">
      {error && <div className="text-sm text-destructive">{error}</div>}

      <div className="space-y-1">
        <Label htmlFor="edit-name">デッキ名</Label>
        <Input id="edit-name" value={name} onChange={(e) => setName(e.target.value)} />
      </div>

      <div className="space-y-1">
        <Label htmlFor="edit-desc">説明（任意）</Label>
        <Textarea
          id="edit-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </div>

      <div className="space-y-2">
        <Label>テストモード</Label>
        <div className="flex flex-col gap-2">
          {AVAILABLE_MODES.map((m) => {
            const checked = testModes.includes(m);
            const locked = original.includes(m); // 既存モードは削除不可
            return (
              <label
                key={m}
                className={`flex items-center gap-2 text-sm ${locked ? "" : "cursor-pointer"}`}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={locked}
                  onChange={(e) => toggleMode(m, e.target.checked)}
                  className="h-4 w-4"
                />
                <span>
                  {modeLabel(m)}（{m}）
                </span>
                {locked && (
                  <span className="text-xs text-muted-foreground">追加済み・削除不可</span>
                )}
              </label>
            );
          })}
        </div>
        <p className="text-xs text-muted-foreground">
          モードは追加のみできます。学習済みの進捗を壊さないため、既存モードの削除はできません。
          リスニングを一時的に止めたい場合は、設定のリスニング出題トグルを使ってください。
        </p>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="space-y-1">
          <Label htmlFor="edit-new">1日の新規上限</Label>
          <Input
            id="edit-new"
            type="number"
            inputMode="numeric"
            min="0"
            value={dailyNew}
            onChange={(e) => setDailyNew(Number(e.target.value))}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="edit-review">1日の復習上限</Label>
          <Input
            id="edit-review"
            type="number"
            inputMode="numeric"
            min="0"
            value={dailyReview}
            onChange={(e) => setDailyReview(Number(e.target.value))}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="edit-ret">目標定着率 (0-1)</Label>
          <Input
            id="edit-ret"
            type="number"
            step="0.01"
            min="0.7"
            max="0.99"
            value={retention}
            onChange={(e) => setRetention(Number(e.target.value))}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="edit-max">最大復習間隔（日）</Label>
          <Input
            id="edit-max"
            type="number"
            inputMode="numeric"
            min="1"
            value={maxInterval}
            onChange={(e) => setMaxInterval(Number(e.target.value))}
          />
        </div>
      </div>

      <div className="space-y-2 rounded-md border p-3">
        <label className="flex cursor-pointer items-center gap-2 text-sm font-medium">
          <input
            type="checkbox"
            checked={studyTargetOn}
            onChange={(e) => setStudyTargetOn(e.target.checked)}
            className="h-4 w-4"
          />
          1日の学習量の目安で新規を自動調整する
        </label>
        {studyTargetOn && (
          <div className="space-y-1">
            <Label htmlFor="edit-study">目安の枚数（新規＋復習）</Label>
            <Input
              id="edit-study"
              type="number"
              inputMode="numeric"
              min="0"
              value={studyTarget}
              onChange={(e) => setStudyTarget(Number(e.target.value))}
            />
          </div>
        )}
        <p className="text-xs text-muted-foreground">
          復習が多い日は、新規＋復習がこの枚数を超えないよう新規カードを自動で減らします。
          復習は常に表示されます（絞られるのは新規のみ）。オフの場合、新規は新規上限どおり出題されます。
        </p>
      </div>

      <div className="flex justify-end gap-2 pt-1">
        <Button type="button" variant="outline" onClick={onCancel} disabled={saving}>
          キャンセル
        </Button>
        <Button type="button" onClick={handleSaveClick} disabled={saving}>
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>

      <ConfirmDialog
        open={confirmOpen}
        title="デッキ設定を保存"
        message={confirmMessage}
        confirmLabel="保存する"
        destructive={addedModes.length > 0}
        onConfirm={() => void handleConfirmedSave()}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}

/** 確認ダイアログの本文。追加モードがあれば不可逆である旨を明示する。 */
function buildConfirmMessage(addedModeLabels: string[]): string {
  if (addedModeLabels.length === 0) {
    return "この内容でデッキ設定を保存します。";
  }
  return (
    "この内容でデッキ設定を保存します。\n\n" +
    `追加するテストモード：${addedModeLabels.join("、")}\n` +
    "※ 追加したテストモードは後から削除できません。"
  );
}
