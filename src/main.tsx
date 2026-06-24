import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { router } from "@/router";
import { useAppStore } from "@/store/appStore";
import { logger } from "@/lib/logger";
import "@/index.css";

// テーマ等のアプリ状態を起動時に読み込む
void useAppStore.getState().hydrate();
void logger.info("Frontend started");

// 画面遷移をログ出力（タブ遷移などの追跡用）
router.subscribe("onResolved", (e) => {
  void logger.debug(`Navigate: ${e.toLocation.pathname}`);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>,
);
