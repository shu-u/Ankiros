import { Volume2 } from "lucide-react";
import type { SpeechLang } from "@/lib/useSpeech";

interface SpeakButtonProps {
  text: string;
  lang: SpeechLang;
  speak: (text: string, lang: SpeechLang) => void;
  speakingText: string | null;
  supported: boolean;
}

export function SpeakButton({
  text,
  lang,
  speak,
  speakingText,
  supported,
}: SpeakButtonProps) {
  if (!supported) return null;
  const isActive = speakingText === text;
  return (
    <button
      type="button"
      onClick={() => speak(text, lang)}
      className={`inline-flex shrink-0 items-center justify-center rounded p-1 transition-colors hover:bg-accent ${
        isActive
          ? "text-primary"
          : "text-muted-foreground hover:text-foreground"
      }`}
      aria-label="読み上げ"
    >
      <Volume2 size={16} className={isActive ? "animate-pulse" : ""} />
    </button>
  );
}
