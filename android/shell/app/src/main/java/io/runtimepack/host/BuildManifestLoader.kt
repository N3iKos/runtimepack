package io.runtimepack.host

import android.content.Context
import java.io.InputStreamReader

data class AppManifest(
    val name: String,
    val packageName: String,
    val versionName: String,
    val versionCode: Int
)

data class RuntimeManifestData(
    val id: String,
    val version: String,
    val family: String,
    val runtimeApi: Int,
    val capabilities: List<String>
)

data class EngineManifestData(
    val id: String,
    val confidence: String = "EXACT",
    val overridden: Boolean = false,
    val detectedVersion: String? = null
)

data class BuildManifestData(
    val schemaVersion: Int,
    val app: AppManifest,
    val runtime: RuntimeManifestData,
    val contentMode: String,
    val engine: EngineManifestData? = null,
    val flavor: String? = null,
    val buildTimestamp: String? = null,
    val contentHash: String? = null,
    val sourceHash: String? = null,
    val overlayMode: String = "full",
    val vnmInputGuard: String = "off"
)

object BuildManifestLoader {
    fun parse(jsonText: String): BuildManifestData {
        val root = MiniJson.parseObjectOrThrow(jsonText)

        val appObj = root.getObject("app")
            ?: throw IllegalArgumentException("Missing required object 'app'")
        val appName = appObj.getString("name")
            ?: throw IllegalArgumentException("Missing required 'app.name'")
        val appPkg = appObj.getString("package")
            ?: throw IllegalArgumentException("Missing required 'app.package'")
        val app = AppManifest(
            name = appName,
            packageName = appPkg,
            versionName = appObj.getString("version_name") ?: "1.0.0",
            versionCode = appObj.getInt("version_code") ?: 1
        )

        val runtimeObj = root.getObject("runtime")
            ?: throw IllegalArgumentException("Missing required object 'runtime'")
        val runtimeId = runtimeObj.getString("id")
            ?: throw IllegalArgumentException("Missing required 'runtime.id'")
        val runtimeFamily = runtimeObj.getString("family")
            ?: throw IllegalArgumentException("Missing required 'runtime.family'")
        val caps = runtimeObj.getArray("capabilities")?.strings() ?: emptyList()
        val runtime = RuntimeManifestData(
            id = runtimeId,
            version = runtimeObj.getString("version") ?: "",
            family = runtimeFamily,
            runtimeApi = runtimeObj.getInt("runtime_api") ?: 1,
            capabilities = caps
        )

        val engineObj = root.getObject("engine")
        val engine = if (engineObj != null) {
            EngineManifestData(
                id = engineObj.getString("id") ?: "unknown",
                confidence = engineObj.getString("confidence") ?: "EXACT",
                overridden = engineObj.getBoolean("overridden") ?: false,
                detectedVersion = engineObj.getString("detected_version")
            )
        } else null

        val contentObj = root.getObject("content")
            ?: throw IllegalArgumentException("Missing required object 'content'")
        val contentMode = contentObj.getString("mode") ?: "packed"
        val contentHash = contentObj.getString("content_hash")

        val flavor = root.getString("flavor")
        val buildTimestamp = root.getString("build_timestamp")

        val provObj = root.getObject("provenance")
        val sourceHash = provObj?.getString("source_hash")

        val overlayMode = root.getString("overlay_mode") ?: "full"
        val vnmInputGuard = root.getString("vnm_input_guard") ?: "off"

        return BuildManifestData(
            schemaVersion = root.getInt("schema_version") ?: 1,
            app = app,
            runtime = runtime,
            contentMode = contentMode,
            engine = engine,
            flavor = flavor,
            buildTimestamp = buildTimestamp,
            contentHash = contentHash,
            sourceHash = sourceHash,
            overlayMode = overlayMode,
            vnmInputGuard = vnmInputGuard
        )
    }

    fun load(context: Context): BuildManifestData {
        val assetManager = context.assets
        val inputStream = assetManager.open("build-manifest.json")
        val jsonText = InputStreamReader(inputStream).readText()
        return parse(jsonText)
    }
}
