import { commands, type LogLevel } from "@/bindings";

export const log = async (level: LogLevel, message: string): Promise<void> => {
  try {
    await commands.log(level, message);
  } catch (error) {
    // Tauri コマンドが失敗した場合は console にフォールバック
    console.error("Failed to log via Tauri:", error);
    console.log(`[${level}] ${message}`);
  }
};

export const logger = {
  error: (message: string) => log("ERROR", message),
  warn: (message: string) => log("WARN", message),
  info: (message: string) => log("INFO", message),
  debug: (message: string) => log("DEBUG", message),
  verbose: (message: string) => log("VERBOSE", message),
};
