package com.plugin.tts

import android.app.Activity
import android.os.Bundle
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.Locale

@InvokeArg
class SpeakArgs {
    lateinit var text: String
    var lang: String? = null
    var volume: Float? = null
}

@TauriPlugin
class TtsPlugin(private val activity: Activity) : Plugin(activity), TextToSpeech.OnInitListener {
    private var tts: TextToSpeech? = null
    private var ready = false

    override fun load(webView: WebView) {
        super.load(webView)
        tts = TextToSpeech(activity, this)
    }

    override fun onInit(status: Int) {
        ready = status == TextToSpeech.SUCCESS
        if (!ready) return
        tts?.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {}

            override fun onDone(utteranceId: String?) {
                emitEnd(utteranceId)
            }

            @Deprecated("Deprecated in API 21, but still invoked on older devices")
            override fun onError(utteranceId: String?) {
                emitEnd(utteranceId)
            }
        })
    }

    private fun emitEnd(utteranceId: String?) {
        val data = JSObject()
        data.put("id", utteranceId ?: "")
        trigger("speakEnd", data)
    }

    @Command
    fun speak(invoke: Invoke) {
        val args = invoke.parseArgs(SpeakArgs::class.java)
        val engine = tts
        if (engine == null || !ready) {
            invoke.reject("TTS engine not ready")
            return
        }
        engine.language = localeFromTag(args.lang)
        // 音量は発話ごとに Bundle で指定（未指定なら端末既定の 1.0）
        val params = Bundle().apply {
            args.volume?.let { putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, it) }
        }
        // utteranceId にテキストを使い、speakEnd で照合して話中状態を解除する
        engine.speak(args.text, TextToSpeech.QUEUE_FLUSH, params, args.text)
        invoke.resolve()
    }

    @Command
    fun stop(invoke: Invoke) {
        tts?.stop()
        invoke.resolve()
    }

    @Command
    fun isAvailable(invoke: Invoke) {
        val ret = JSObject()
        ret.put("available", ready)
        invoke.resolve(ret)
    }

    private fun localeFromTag(tag: String?): Locale {
        return when (tag) {
            "zh-CN" -> Locale.SIMPLIFIED_CHINESE
            "ja-JP" -> Locale.JAPANESE
            "en-US" -> Locale.US
            null -> Locale.getDefault()
            else -> Locale.forLanguageTag(tag)
        }
    }
}
