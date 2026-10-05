package io.runtimepack.host

data class RuntimeInputEvent(
    val type: String, // key_down, key_up, pointer_down, pointer_move, pointer_up
    val code: String,
    val timestampNs: Long = System.nanoTime(),
    val x: Float = 0f,
    val y: Float = 0f
)

interface InputEventListener {
    fun onInputEvent(event: RuntimeInputEvent)
}

class InputService {
    private val listeners = mutableListOf<InputEventListener>()

    fun addListener(listener: InputEventListener) {
        listeners.add(listener)
    }

    fun removeListener(listener: InputEventListener) {
        listeners.remove(listener)
    }

    fun dispatch(event: RuntimeInputEvent) {
        for (listener in listeners) {
            listener.onInputEvent(event)
        }
    }
}
