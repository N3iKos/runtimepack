package io.runtimepack.host

object RuntimeBuildInfo {
    @Volatile
    var RUNTIME_INTEGRITY_SHA256: String = ""

    @Volatile
    var RUNTIME_INTEGRITY_PATHS: List<String> = emptyList()

    @Volatile
    var BUILD_FLAVOR: String = "beta"

    @Volatile
    var SCRIPT_COMPLETION_HOOKS: List<String> = emptyList()
}
