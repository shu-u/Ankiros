import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Layout } from "@/components/Layout";
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
