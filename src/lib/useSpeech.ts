import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, addPluginListener, type PluginListener } from "@tauri-apps/api/core";
import { isAndroid } from "@/lib/platform";
import { useAppStore } from "@/store/appStore";

export type SpeechLang = "zh-CN" | "ja-JP" | "en-US";

// 実行プラットフォームはアプリ起動中に変化しないため、モジュールロード時に確定。
// これにより useSpeech 内の分岐は常に同じ経路を通り、Hooks のルールを満たす。
const ANDROID = isAndroid();

export function useSpeech() {
  const [supported, setSupported] = useState(false);
  const [speakingText, setSpeakingText] = useState<string | null>(null);
  const voicesRef = useRef<SpeechSynthesisVoice[]>([]);

  // 発話ごとに最新の音量を参照する。speak/stop の参照を安定させるため
  // state を直接読まず ref 経由にする（voicesRef と同じ方針）。
  const volume = useAppStore((s) => s.volume);
  const volumeRef = useRef(volume);
  useEffect(() => {
    volumeRef.current = volume;
  }, [volume]);

  // --- デスクトップ: Web Speech API（従来実装のまま・挙動不変） ---
  useEffect(() => {
    if (ANDROID) return;
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

  // --- Android: ネイティブ TTS プラグイン (plugin:tts) ---
  useEffect(() => {
    if (!ANDROID) return;
    // TextToSpeech は実機でほぼ常に利用可能。エンジン初期化は非同期で
    // わずかに遅れるため、可否確認を待たず楽観的にボタンを表示する。
    setSupported(true);

    let listener: PluginListener | undefined;
    let cancelled = false;
    void (async () => {
      try {
        // 発話完了イベントで話中表示（パルス）を解除する
        const l = await addPluginListener<{ id: string }>(
          "tts",
          "speakEnd",
          (payload) => {
            setSpeakingText((cur) => (cur !== null && cur === payload.id ? null : cur));
          },
        );
        if (cancelled) l.unregister();
        else listener = l;
      } catch {
        // リスナー登録に失敗しても発話自体は可能なので握りつぶす
      }
    })();

    return () => {
      cancelled = true;
      listener?.unregister();
      void invoke("plugin:tts|stop").catch(() => {});
    };
  }, []);

  // rate: 読み上げ速度 (1 = 等速)。リスニングモードの低速再生ボタンで 0.25〜1 を渡す。
  const speak = useCallback((text: string, lang: SpeechLang, rate = 1) => {
    if (ANDROID) {
      setSpeakingText(text);
      void invoke("plugin:tts|speak", {
        text,
        lang,
        volume: volumeRef.current,
        rate,
      }).catch(() => setSpeakingText(null));
      return;
    }

    if (!("speechSynthesis" in window)) return;
    window.speechSynthesis.cancel();

    const utter = new SpeechSynthesisUtterance(text);
    utter.lang = lang;
    utter.volume = volumeRef.current;
    utter.rate = rate;

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
    if (ANDROID) {
      setSpeakingText(null);
      void invoke("plugin:tts|stop").catch(() => {});
      return;
    }

    if (!("speechSynthesis" in window)) return;
    window.speechSynthesis.cancel();
    setSpeakingText(null);
  }, []);

  return { supported, speakingText, speak, stop };
}
