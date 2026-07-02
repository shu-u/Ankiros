import { createRootRoute, createRoute, createRouter, redirect } from "@tanstack/react-router";
import { Layout } from "@/components/Layout";
import { useSessionStore } from "@/store/sessionStore";
import { HomePage } from "@/routes/Home";
import { DecksPage } from "@/routes/Decks";
import { DeckDetailPage } from "@/routes/DeckDetail";
import { StudyPage } from "@/routes/Study";
import { ResultPage } from "@/routes/Result";
import { CardsPage } from "@/routes/Cards";
import { CardDetailPage } from "@/routes/CardDetail";
import { SettingsPage } from "@/routes/Settings";

const rootRoute = createRootRoute({ component: Layout });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});

const decksRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks",
  // 学習中（未完了セッション）なら、デッキ一覧タブを開いたとき学習画面へ戻す。
  // 明示的な中断（reset）でセッションが消えるまで学習中の状態を保持する。
  beforeLoad: () => {
    const s = useSessionStore.getState();
    if (s.deckId !== null && s.currentCard !== null && !s.isComplete) {
      throw redirect({
        to: "/decks/$deckId/study",
        params: { deckId: s.deckId },
      });
    }
  },
  component: DecksPage,
});

const deckDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks/$deckId",
  component: DeckDetailPage,
});

const studyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks/$deckId/study",
  component: StudyPage,
});

const resultRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks/$deckId/study/result",
  component: ResultPage,
});

const cardsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks/$deckId/cards",
  component: CardsPage,
});

const cardDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/decks/$deckId/cards/$cardId",
  component: CardDetailPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  decksRoute,
  deckDetailRoute,
  studyRoute,
  resultRoute,
  cardsRoute,
  cardDetailRoute,
  settingsRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
