package io.runtimepack.host

object RuntimeBuildInfo {
    @Volatile
    var RUNTIME_INTEGRITY_SHA256: String = ""

    @Volatile
    var BUILD_FLAVOR: String = "beta"
}
