import { useCallback, useEffect, useRef, useState } from "react";

export type SpeechLang = "zh-CN" | "ja-JP" | "en-US";

export function useSpeech() {
  const [supported, setSupported] = useState(false);
  const [speakingText, setSpeakingText] = useState<string | null>(null);
  const voicesRef = useRef<SpeechSynthesisVoice[]>([]);

  useEffect(() => {
    if (!("speechSynthesis" in window)) return;
    setSupported(true);

    const updateVoices = () => {
      voicesRef.current = window.speechSynthesis.getVoices();
    };
    updateVoices();
    // Android WebView では voiceschanged イベントで遅延ロードされる
    window.speechSynthesis.addEventListener("voiceschanged", updateVoices);

    return () => {
      window.speechSynthesis.removeEventListener("voiceschanged", updateVoices);
      window.speechSynthesis.cancel();
    };
  }, []);

  const speak = useCallback((text: string, lang: SpeechLang) => {
    if (!("speechSynthesis" in window)) return;
    window.speechSynthesis.cancel();

    const utter = new SpeechSynthesisUtterance(text);
    utter.lang = lang;

    const voices =
      voicesRef.current.length > 0
        ? voicesRef.current
        : window.speechSynthesis.getVoices();
    // 完全一致 → 言語コード前方一致の順で音声を選択
    const langPrefix = lang.split("-")[0];
    const voice =
      voices.find((v) => v.lang === lang) ??
      voices.find((v) => v.lang.startsWith(langPrefix));
    if (voice) utter.voice = voice;

    utter.onstart = () => setSpeakingText(text);
    utter.onend = () => setSpeakingText(null);
    utter.onerror = () => setSpeakingText(null);

    window.speechSynthesis.speak(utter);
  }, []);

  const stop = useCallback(() => {
    if (!("speechSynthesis" in window)) return;
    window.speechSynthesis.cancel();
    setSpeakingText(null);
  }, []);

  return { supported, speakingText, speak, stop };
}
