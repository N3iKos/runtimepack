package io.runtimepack.host

import android.util.Log

enum class HealthMarker {
    HOST_CREATED,
    CONTENT_MOUNTED,
    RUNTIME_CREATED,
    GAME_STARTED,
    FIRST_FRAME,
    SAVE_READY;

    fun emit() {
        Log.i("RuntimePackHealth", "MARKER: $name (timestamp: ${System.currentTimeMillis()})")
    }
}
