package io.runtimepack.host

import android.content.Context
import org.json.JSONObject
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

data class BuildManifestData(
    val schemaVersion: Int,
    val app: AppManifest,
    val runtime: RuntimeManifestData,
    val contentMode: String
)

object BuildManifestLoader {
    fun load(context: Context): BuildManifestData {
        val assetManager = context.assets
        val inputStream = assetManager.open("build-manifest.json")
        val jsonText = InputStreamReader(inputStream).readText()
        val root = JSONObject(jsonText)

        val appObj = root.getJSONObject("app")
        val app = AppManifest(
            name = appObj.getString("name"),
            packageName = appObj.getString("package"),
            versionName = appObj.optString("version_name", "1.0.0"),
            versionCode = appObj.optInt("version_code", 1)
        )

        val runtimeObj = root.getJSONObject("runtime")
        val capsJson = runtimeObj.optJSONArray("capabilities")
        val caps = mutableListOf<String>()
        if (capsJson != null) {
            for (i in 0 until capsJson.length()) {
                caps.add(capsJson.getString(i))
            }
        }

        val runtime = RuntimeManifestData(
            id = runtimeObj.getString("id"),
            version = runtimeObj.getString("version"),
            family = runtimeObj.getString("family"),
            runtimeApi = runtimeObj.optInt("runtime_api", 1),
            capabilities = caps
        )

        val contentObj = root.getJSONObject("content")
        val contentMode = contentObj.optString("mode", "packed")

        return BuildManifestData(
            schemaVersion = root.optInt("schema_version", 1),
            app = app,
            runtime = runtime,
            contentMode = contentMode
        )
    }
}
