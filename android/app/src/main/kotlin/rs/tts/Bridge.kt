package rs.tts

import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener

class Bridge(@JvmField val backendId: Int) :
    UtteranceProgressListener(), TextToSpeech.OnInitListener {
    external override fun onInit(status: Int)
    external override fun onStart(utteranceId: String)
    external override fun onStop(utteranceId: String, interrupted: Boolean)
    external override fun onDone(utteranceId: String)
    @Suppress("OVERRIDE_DEPRECATION")
    external override fun onError(utteranceId: String)
}
