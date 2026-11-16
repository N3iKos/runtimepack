package io.runtimepack.host

import android.app.Activity
import android.os.Bundle
import android.view.View

class BootstrapActivity : Activity() {
    private lateinit var runtimeHost: RuntimeHost

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val app = application as RuntimeApplication
        val runtimeContext = RuntimeContext(
            application = app,
            contentService = app.contentService,
            saveService = app.saveService,
            inputService = app.inputService
        )

        val surfaceView = View(this)
        setContentView(surfaceView)

        runtimeHost = RuntimeHost(runtimeContext)
        runtimeHost.initialize(app.manifest, surfaceView)
    }

    override fun onPause() {
        super.onPause()
        runtimeHost.onPause()
    }

    override fun onResume() {
        super.onResume()
        runtimeHost.onResume()
    }

    override fun onDestroy() {
        super.onDestroy()
        runtimeHost.onDestroy()
    }
}
