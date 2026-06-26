import { create } from "zustand";
import type { SessionCard } from "@/bindings";

export type Rating = "again" | "hard" | "good" | "easy";

export interface SessionResults {
  total: number;
  againCards: SessionCard[];
  hardCount: number;
  goodCount: number;
  easyCount: number;
}

/** 学習中に表示する進捗（再出題で水増ししない、ユニット単位の到達度）。 */
export interface SessionProgress {
  /** セッション開始時の新規ユニット総数 */
  newTotal: number;
  /** セッション開始時の復習ユニット総数（learning/relearning/review を含む） */
  reviewTotal: number;
  /** 同日再出題が確定せず「今日ぶんが完了」した新規ユニット数 */
  newDone: number;
  /** 同上の復習ユニット数 */
  reviewDone: number;
}

interface SessionStore {
  deckId: string | null;
  queue: SessionCard[];
  currentCard: SessionCard | null;
  results: SessionResults;
  progress: SessionProgress;
  /** ユニットキー → 開始時が新規だったか（進捗の分類用） */
  initialIsNew: Record<string, boolean>;
  isComplete: boolean;
  /** セッション中に編集したユーザーメモ (cardId → notes)。
   * 同日再出題でキューの古いカードオブジェクトが再表示されても最新メモを反映するため。 */
  noteEdits: Record<string, string>;

  /** 新しいセッションを開始する（get_session_queue 呼び出し直後に使用） */
  initSession: (deckId: string, queue: SessionCard[]) => void;
  /** 回答を記録し、キューを進める (spec §6.2) */
  recordAnswer: (
    card: SessionCard,
    rating: Rating,
    shouldRequeue: boolean,
  ) => void;
  /** セッション中のユーザーメモ編集を記録する */
  setNoteEdit: (cardId: string, notes: string) => void;
  reset: () => void;
}

function emptyResults(): SessionResults {
  return { total: 0, againCards: [], hardCount: 0, goodCount: 0, easyCount: 0 };
}

function emptyProgress(): SessionProgress {
  return { newTotal: 0, reviewTotal: 0, newDone: 0, reviewDone: 0 };
}

const key = (c: SessionCard) => `${c.card.id}__${c.mode}`;
const isNewState = (c: SessionCard) => c.srs_state === "new";

export const useSessionStore = create<SessionStore>((set) => ({
  deckId: null,
  queue: [],
  currentCard: null,
  results: emptyResults(),
  progress: emptyProgress(),
  initialIsNew: {},
  isComplete: false,
  noteEdits: {},

  initSession: (deckId, queue) => {
    const newTotal = queue.filter(isNewState).length;
    const initialIsNew: Record<string, boolean> = {};
    for (const c of queue) initialIsNew[key(c)] = isNewState(c);
    set({
      deckId,
      queue,
      currentCard: queue.length > 0 ? queue[0] : null,
      results: emptyResults(),
      progress: {
        newTotal,
        reviewTotal: queue.length - newTotal,
        newDone: 0,
        reviewDone: 0,
      },
      initialIsNew,
      isComplete: queue.length === 0,
      noteEdits: {},
    });
  },

  recordAnswer: (card, rating, shouldRequeue) =>
    set((s) => {
      // 集計を更新
      const results: SessionResults = {
        total: s.results.total + 1,
        againCards: [...s.results.againCards],
        hardCount: s.results.hardCount + (rating === "hard" ? 1 : 0),
        goodCount: s.results.goodCount + (rating === "good" ? 1 : 0),
        easyCount: s.results.easyCount + (rating === "easy" ? 1 : 0),
      };
      if (rating === "again") {
        if (!results.againCards.some((c) => key(c) === key(card))) {
          results.againCards.push(card);
        }
      }

      // 進捗: 同日再出題が無い（＝今日ぶんが完了した）ユニットだけ加算する。
      // 再出題されるユニットはまだ「学習中」なので加算しない＝分母も水増しされない。
      const wasNew = s.initialIsNew[key(card)] ?? false;
      const progress: SessionProgress = {
        ...s.progress,
        newDone: s.progress.newDone + (!shouldRequeue && wasNew ? 1 : 0),
        reviewDone: s.progress.reviewDone + (!shouldRequeue && !wasNew ? 1 : 0),
      };

      // キューの先頭（現在のカード）を取り除く
      const rest = s.queue.slice(1);
      // 同日再出題の場合は末尾へ再追加 (spec §6.2)。
      // 最新のユーザーメモを反映し、かつ新しい参照にすることで再表示時に
      // 問題フェーズへリセットされるようにする（同一カードのみのキュー対策）。
      const requeued: SessionCard = {
        ...card,
        card: { ...card.card, user_notes: s.noteEdits[card.card.id] ?? card.card.user_notes },
      };
      const nextQueue = shouldRequeue ? [...rest, requeued] : rest;

      return {
        results,
        progress,
        queue: nextQueue,
        currentCard: nextQueue.length > 0 ? nextQueue[0] : null,
        isComplete: nextQueue.length === 0,
      };
    }),

  setNoteEdit: (cardId, notes) =>
    set((s) => ({ noteEdits: { ...s.noteEdits, [cardId]: notes } })),

  reset: () =>
    set({
      deckId: null,
      queue: [],
      currentCard: null,
      results: emptyResults(),
      progress: emptyProgress(),
      initialIsNew: {},
      isComplete: false,
      noteEdits: {},
    }),
}));
