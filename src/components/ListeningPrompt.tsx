import { Volume2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ExampleSentence } from "@/bindings";
import type { SpeechLang } from "@/lib/useSpeech";

/** 例文の読み上げ速度（倍速）。0.25〜1倍を用意する。 */
const SPEEDS = [0.25, 0.5, 0.75, 1] as const;

interface Props {
  /** 出題語（漢字）。リスニングでは画面に文字は出さず音声のみ再生する。 */
  word: string;
  /** 出題時に固定されたランダム例文。無い場合は単語のみ。 */
  example: ExampleSentence | null;
  speak: (text: string, lang: SpeechLang, rate?: number) => void;
  speakingText: string | null;
  supported: boolean;
}

/**
 * リスニングモードの出題UI。漢字を隠し、単語＋例文（速度別）の発話ボタンだけを提示する。
 * 例文は出題時に1つ固定され、速度違いで何度でも同じ文を再生できる。
 */
export function ListeningPrompt({ word, example, speak, speakingText, supported }: Props) {
  if (!supported) {
    return (
      <p className="text-sm text-muted-foreground">
        この環境では音声読み上げが利用できません。設定でリスニング出題をオフにできます。
      </p>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">音声を聞いて意味を答えてください</p>

      <div className="space-y-1">
        <div className="text-xs text-muted-foreground">単語</div>
        <Button
          type="button"
          variant="outline"
          className="h-12 gap-2"
          onClick={() => speak(word, "zh-CN")}
        >
          <Volume2
            className={`h-5 w-5 ${speakingText === word ? "animate-pulse text-primary" : ""}`}
          />
          単語を再生
        </Button>
      </div>

      {example && (
        <div className="space-y-1">
          <div className="text-xs text-muted-foreground">例文（速度を選んで再生）</div>
          <div className="flex flex-wrap justify-center gap-2">
            {SPEEDS.map((s) => (
              <Button
                key={s}
                type="button"
                variant="outline"
                className="h-12 gap-1 tabular-nums"
                onClick={() => speak(example.text, "zh-CN", s)}
              >
                <Volume2 className="h-4 w-4" />
                {s}倍
              </Button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
