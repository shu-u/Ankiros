import { create } from "zustand";
import { call, commands } from "@/lib/api";

export type Theme = "light" | "dark";

interface AppStore {
  theme: Theme;
  lastUsedDeckId: string | null;
  /** TTS 読み上げ音量 (0.0〜1.0) */
  volume: number;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setTheme: (theme: Theme) => Promise<void>;
  /** 音量を即時反映（UI のみ・永続化しない）。スライダー操作中に使う。 */
  setVolume: (volume: number) => void;
  /** 音量を DB に保存（操作確定時に使う）。 */
  saveVolume: (volume: number) => Promise<void>;
  setLastUsedDeckId: (id: string | null) => void;
}

function applyThemeClass(theme: Theme) {
  const root = document.documentElement;
  if (theme === "dark") root.classList.add("dark");
  else root.classList.remove("dark");
}

function clampVolume(v: number): number {
  if (!Number.isFinite(v)) return 1;
  return Math.min(1, Math.max(0, v));
}

export const useAppStore = create<AppStore>((set) => ({
  theme: "light",
  lastUsedDeckId: null,
  volume: 1,
  hydrated: false,

  hydrate: async () => {
    try {
      const state = await call(commands.getAppState());
      const theme = (state.theme as Theme) === "dark" ? "dark" : "light";
      applyThemeClass(theme);
      set({
        theme,
        lastUsedDeckId: state.last_used_deck_id,
        volume: clampVolume(state.tts_volume),
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

  setVolume: (volume) => set({ volume: clampVolume(volume) }),

  saveVolume: async (volume) => {
    const v = clampVolume(volume);
    set({ volume: v });
    await call(commands.updateAppState("tts_volume", String(v)));
  },

  setLastUsedDeckId: (id) => set({ lastUsedDeckId: id }),
}));
