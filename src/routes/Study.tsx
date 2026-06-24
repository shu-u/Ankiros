import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import type { IntervalPreview } from "@/bindings";
import { call, commands } from "@/lib/api";
import { useSessionStore, type Rating } from "@/store/sessionStore";
import { useAppStore } from "@/store/appStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { PinyinDiff } from "@/components/PinyinDiff";
import { analyzePinyin, type PinyinStatus } from "@/lib/pinyin";
import { logger } from "@/lib/logger";
import { Loading, ErrorBox, modeLabel } from "@/components/common";
import { useSpeech } from "@/lib/useSpeech";
import { SpeakButton } from "@/components/SpeakButton";

type Phase = "question" | "answer";

const RATINGS: { rating: Rating; label: string; key: string }[] = [
  { rating: "again", label: "Again", key: "1" },
  { rating: "hard", label: "Hard", key: "2" },
  { rating: "good", label: "Good", key: "3" },
  { rating: "easy", label: "Easy", key: "4" },
];

export function StudyPage() {
  const { deckId } = useParams({ strict: false }) as { deckId: string };
  const navigate = useNavigate();
  const setLastUsedDeckId = useAppStore((s) => s.setLastUsedDeckId);

  const { currentCard, isComplete, initSession, recordAnswer, setNoteEdit } =
    useSessionStore();

  const { supported: speechSupported, speakingText, speak, stop: stopSpeech } = useSpeech();

  const [ready, setReady] = useState(false);
  const [hadCards, setHadCards] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [phase, setPhase] = useState<Phase>("question");
  const [input, setInput] = useState("");
  const [preview, setPreview] = useState<IntervalPreview | null>(null);
  const [userNotes, setUserNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // セッション開始（マウント時に1回）
  useEffect(() => {
    let cancelled = false;
    setLastUsedDeckId(deckId);
    call(commands.getSessionQueue(deckId))
      .then((queue) => {
        if (cancelled) return;
        initSession(deckId, queue);
        setHadCards(queue.length > 0);
        setReady(true);
        void logger.info(`Study session started: deck=${deckId}, queue=${queue.length}`);
      })
      .catch((e) => !cancelled && setLoadError(e instanceof Error ? e.message : String(e)));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deckId]);

  // カードが変わったら問題フェーズへリセット
  useEffect(() => {
    stopSpeech();
    setPhase("question");
    setInput("");
    setPreview(null);
    // セッション中に編集済みのメモがあればそれを優先する
    const edits = useSessionStore.getState().noteEdits;
    setUserNotes(
      currentCard
        ? (edits[currentCard.card.id] ?? currentCard.card.user_notes)
        : "",
    );
    setTimeout(() => inputRef.current?.focus(), 0);
  }, [currentCard, stopSpeech]);

  // 完了したら結果画面へ
  useEffect(() => {
    if (ready && hadCards && isComplete) {
      void logger.info("Study session complete");
      navigate({ to: "/decks/$deckId/study/result", params: { deckId } });
    }
  }, [ready, hadCards, isComplete, deckId, navigate]);

  const flip = useCallback(async () => {
    if (!currentCard || input.trim() === "") return;
    void logger.debug(
      `Flip: card=${currentCard.card.id} mode=${currentCard.mode} input="${input}"`,
    );
    setPhase("answer");
    try {
      const p = await call(
        commands.previewReview(currentCard.card.id, deckId, currentCard.mode),
      );
      setPreview(p);
    } catch {
      /* プレビュー失敗時はラベルなしで継続 */
    }
  }, [currentCard, input, deckId]);

  const rate = useCallback(
    async (rating: Rating) => {
      if (!currentCard || submitting) return;
      setSubmitting(true);
      const card = currentCard;
      try {
        const res = await call(
          commands.submitReview(card.card.id, deckId, card.mode, rating),
        );
        void logger.debug(
          `Rate: card=${card.card.id} mode=${card.mode} rating=${rating} requeue=${res.should_requeue}`,
        );
        recordAnswer(card, rating, res.should_requeue);
      } catch (e) {
        alert(e instanceof Error ? e.message : String(e));
      } finally {
        setSubmitting(false);
      }
    },
    [currentCard, deckId, recordAnswer, submitting],
  );

  // キーボードショートカット (spec §10)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        void logger.info("Study session aborted (Esc)");
        navigate({ to: "/" });
        return;
      }
      if (phase === "question") {
        if (e.key === "Enter") {
          e.preventDefault();
          void flip();
        }
      } else {
        const found = RATINGS.find((r) => r.key === e.key);
        if (found) {
          e.preventDefault();
          void rate(found.rating);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [phase, flip, rate, navigate]);

  if (loadError) return <ErrorBox message={loadError} />;
  if (!ready) return <Loading label="セッションを準備中…" />;

  if (!hadCards) {
    return (
      <div className="space-y-4 text-center">
        <h1 className="text-2xl font-bold">学習するカードがありません</h1>
        <p className="text-muted-foreground">
          今日の新規・復習カードはすべて完了しているか、カードが登録されていません。
        </p>
        <Button onClick={() => navigate({ to: "/decks/$deckId", params: { deckId } })}>
          デッキ詳細へ戻る
        </Button>
      </div>
    );
  }

  if (!currentCard) return <Loading />;

  const card = currentCard.card;
  const mode = currentCard.mode;
  const problem = mode === "production" ? card.meaning : card.hanzi;
  const showDiff = mode === "pronunciation" || mode === "production";
  // 声調記号形式・数字形式を同一視して正誤判定する (pronunciation/production)
  const analysis = showDiff ? analyzePinyin(input, card.pinyin_accepted) : null;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div className="flex items-center justify-between">
        <Badge variant="secondary">{modeLabel(mode)}モード</Badge>
        <button
          onClick={() => navigate({ to: "/" })}
          className="text-sm text-muted-foreground hover:underline"
        >
          中断<span className="hidden md:inline">（Esc）</span>
        </button>
      </div>

      {/* 問題 */}
      <div className="rounded-lg border bg-card p-6 text-center md:p-8">
        <div className="mb-2 text-sm text-muted-foreground">
          {mode === "production" ? "意味" : "漢字"}
        </div>
        <div className={`${mode === "production" ? "text-3xl font-semibold" : "hanzi text-5xl font-bold"} flex items-center justify-center gap-2`}>
          <span>{problem}</span>
          <SpeakButton
            text={problem}
            lang={mode === "production" ? "ja-JP" : "zh-CN"}
            speak={speak}
            speakingText={speakingText}
            supported={speechSupported}
          />
        </div>
      </div>

      {phase === "question" ? (
        <div className="space-y-3">
          <Input
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={
              mode === "recognition" ? "意味を入力…" : "ピンインを入力…"
            }
            autoFocus
          />
          <Button className="h-12 w-full" disabled={input.trim() === ""} onClick={() => void flip()}>
            めくる<span className="hidden md:inline">（Enter）</span>
          </Button>
        </div>
      ) : (
        <div className="space-y-5">
          {/* 入力比較 */}
          <div className="space-y-3 rounded-lg border bg-card p-4">
            {showDiff && analysis ? (
              <>
                <PinyinVerdict status={analysis.status} />
                {analysis.status === "correct" ? (
                  // 書式違い（yi1 sheng1 ↔ yī shēng）でも正解なら差分は出さず正解を表示
                  <div className="space-y-1 font-mono text-base">
                    <div className="flex flex-wrap items-baseline gap-x-1">
                      <span className="mr-2 shrink-0 font-sans text-sm text-muted-foreground">
                        あなたの入力：
                      </span>
                      <span className="diff-added">{input}</span>
                    </div>
                    <div className="flex flex-wrap items-baseline gap-x-1">
                      <span className="mr-2 shrink-0 font-sans text-sm text-muted-foreground">
                        正解：
                      </span>
                      <span>{analysis.target}</span>
                    </div>
                  </div>
                ) : (
                  <PinyinDiff user={input} correct={analysis.target} />
                )}
              </>
            ) : (
              <div className="space-y-1 text-base">
                <div className="flex gap-2">
                  <span className="shrink-0 text-sm text-muted-foreground">あなたの入力：</span>
                  <span>{input}</span>
                </div>
                <div className="flex gap-2">
                  <span className="shrink-0 text-sm text-muted-foreground">正解：</span>
                  <span className="font-medium">{card.meaning}</span>
                </div>
              </div>
            )}
          </div>

          {/* カード詳細 */}
          <div className="space-y-3 rounded-lg border bg-card p-4 text-sm">
            <div className="flex gap-2">
              <span className="shrink-0 text-muted-foreground">意味：</span>
              <span className="flex items-center gap-1">
                {card.meaning}
                <SpeakButton
                  text={card.meaning}
                  lang="ja-JP"
                  speak={speak}
                  speakingText={speakingText}
                  supported={speechSupported}
                />
              </span>
            </div>
            <DetailRow label="ピンイン">{card.pinyin_accepted.join(" / ")}</DetailRow>
            {card.example_sentences.length > 0 && (
              <div>
                <div className="mb-1 text-muted-foreground">例文</div>
                <div className="space-y-2">
                  {card.example_sentences.map((ex, i) => (
                    <div key={i} className="rounded bg-accent/40 p-2">
                      <div className="hanzi flex items-center gap-1">
                        <span>{ex.text}</span>
                        <SpeakButton
                          text={ex.text}
                          lang="zh-CN"
                          speak={speak}
                          speakingText={speakingText}
                          supported={speechSupported}
                        />
                      </div>
                      {ex.pinyin && <div className="text-muted-foreground">{ex.pinyin}</div>}
                      {ex.translation && <div>{ex.translation}</div>}
                    </div>
                  ))}
                </div>
              </div>
            )}
            {card.synonyms.length > 0 && (
              <DetailRow label="類義語">{card.synonyms.join("、")}</DetailRow>
            )}
            {card.antonyms.length > 0 && (
              <DetailRow label="対義語">{card.antonyms.join("、")}</DetailRow>
            )}
            {card.ai_notes && <DetailRow label="AIメモ">{card.ai_notes}</DetailRow>}
            <div>
              <div className="mb-1 text-muted-foreground">ユーザーメモ</div>
              <Textarea
                value={userNotes}
                onChange={(e) => setUserNotes(e.target.value)}
                onBlur={() => {
                  void call(commands.updateUserNotes(card.id, deckId, userNotes));
                  setNoteEdit(card.id, userNotes);
                }}
                placeholder="メモを入力…"
                className="min-h-[60px]"
              />
            </div>
          </div>

          {/* 評価ボタン */}
          <div className="grid grid-cols-4 gap-2">
            {RATINGS.map(({ rating, label, key }) => (
              <button
                key={rating}
                disabled={submitting}
                onClick={() => void rate(rating)}
                className="flex min-h-[3.5rem] flex-col items-center justify-center gap-1 rounded-md border py-3 transition-colors hover:bg-accent disabled:opacity-50"
              >
                <span className="font-semibold">{label}</span>
                <span className="hidden text-xs text-muted-foreground md:block">{key}</span>
                <span className="text-xs text-muted-foreground">
                  {preview ? preview[rating] : "…"}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function PinyinVerdict({ status }: { status: PinyinStatus }) {
  const map: Record<PinyinStatus, { label: string; cls: string }> = {
    correct: { label: "正解", cls: "bg-green-200 text-green-900 dark:bg-green-800 dark:text-green-100" },
    tone: {
      label: "声調が違います",
      cls: "bg-amber-200 text-amber-900 dark:bg-amber-700 dark:text-amber-100",
    },
    incorrect: {
      label: "不正解",
      cls: "bg-red-200 text-red-900 dark:bg-red-800 dark:text-red-100",
    },
  };
  const { label, cls } = map[status];
  return (
    <span className={`inline-flex rounded-full px-3 py-0.5 text-sm font-semibold ${cls}`}>
      {label}
    </span>
  );
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex gap-2">
      <span className="shrink-0 text-muted-foreground">{label}：</span>
      <span>{children}</span>
    </div>
  );
}
