package org.deflorta.android

import android.os.Process
import android.os.Bundle
import com.google.androidgamesdk.GameActivity
import com.google.androidgamesdk.gametextinput.State

class DeflortaActivity : GameActivity() {
    companion object {
        init {
            System.loadLibrary("deflorta_android")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // android-activity 0.6 cannot read GameTextInput's initial null buffer.
        // Seed it before Android resumes the native event loop.
        stateChanged(State("", 0, 0, -1, -1), false)
    }

    override fun onDestroy() {
        super.onDestroy()
        Process.killProcess(Process.myPid())
    }
}
