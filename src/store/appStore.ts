import { create } from "zustand";
import { call, commands } from "@/lib/api";

export type Theme = "light" | "dark";

interface AppStore {
  theme: Theme;
  lastUsedDeckId: string | null;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setTheme: (theme: Theme) => Promise<void>;
  setLastUsedDeckId: (id: string | null) => void;
}

function applyThemeClass(theme: Theme) {
  const root = document.documentElement;
  if (theme === "dark") root.classList.add("dark");
  else root.classList.remove("dark");
}

export const useAppStore = create<AppStore>((set) => ({
  theme: "light",
  lastUsedDeckId: null,
  hydrated: false,

  hydrate: async () => {
    try {
      const state = await call(commands.getAppState());
      const theme = (state.theme as Theme) === "dark" ? "dark" : "light";
      applyThemeClass(theme);
      set({
        theme,
        lastUsedDeckId: state.last_used_deck_id,
        hydrated: true,
      });
    } catch {
      applyThemeClass("light");
      set({ hydrated: true });
    }
  },

  setTheme: async (theme) => {
    applyThemeClass(theme);
    set({ theme });
    await call(commands.updateAppState("theme", theme));
  },

  setLastUsedDeckId: (id) => set({ lastUsedDeckId: id }),
}));
