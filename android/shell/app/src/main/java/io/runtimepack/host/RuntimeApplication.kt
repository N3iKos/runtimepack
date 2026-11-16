package io.runtimepack.host

import android.app.Application

class RuntimeApplication : Application() {
    lateinit var manifest: BuildManifestData
        private set
    lateinit var saveService: SaveService
        private set
    lateinit var contentService: ContentService
        private set
    lateinit var inputService: InputService
        private set

    override fun onCreate() {
        super.onCreate()
        manifest = BuildManifestLoader.load(this)
        saveService = InternalStorageSaveService(this)
        contentService = AssetContentService(this, manifest.contentMode)
        inputService = InputService()
        val manifestEngine = manifest.engine?.id ?: manifest.runtime.family
        AssetAuditService.init(filesDir, manifestEngine, this, manifest.sourceHash)
    }
}
