package io.runtimepack.host

import android.view.View

data class RuntimeContext(
    val application: RuntimeApplication,
    val contentService: ContentService,
    val saveService: SaveService,
    val inputService: InputService
)

interface RuntimeBackend {
    fun create(context: RuntimeContext)
    fun mountContent(contentService: ContentService)
    fun attachSurface(surfaceView: View)
    fun start(entryPoint: String)
    fun onInput(event: RuntimeInputEvent)
    fun pause()
    fun resume()
    fun stop()
}

class TestBackend : RuntimeBackend {
    private var context: RuntimeContext? = null

    override fun create(context: RuntimeContext) {
        this.context = context
        HealthMarker.HOST_CREATED.emit()
        HealthMarker.RUNTIME_CREATED.emit()
    }

    override fun mountContent(contentService: ContentService) {
        HealthMarker.CONTENT_MOUNTED.emit()
    }

    override fun attachSurface(surfaceView: View) {
        // Surface attached
    }

    override fun start(entryPoint: String) {
        HealthMarker.GAME_STARTED.emit()
        // Simulate rendering first frame
        HealthMarker.FIRST_FRAME.emit()
        // Verify save system
        context?.saveService?.openWrite("test_slot.sav")?.use {
            it.write("health_check_ok".toByteArray())
        }
        HealthMarker.SAVE_READY.emit()
    }

    override fun onInput(event: RuntimeInputEvent) {
        // Process test input
    }

    override fun pause() {}

    override fun resume() {}

    override fun stop() {}
}

class RuntimeHost(private val context: RuntimeContext) {
    private var backend: RuntimeBackend? = null

    fun initialize(manifest: BuildManifestData, surfaceView: View) {
        backend = when (manifest.runtime.family) {
            "test" -> TestBackend()
            else -> TestBackend() // Default to test backend in shell template
        }

        backend?.apply {
            create(context)
            mountContent(context.contentService)
            attachSurface(surfaceView)
            start("main")
        }
    }

    fun onPause() {
        backend?.pause()
    }

    fun onResume() {
        backend?.resume()
    }

    fun onDestroy() {
        backend?.stop()
    }
}
