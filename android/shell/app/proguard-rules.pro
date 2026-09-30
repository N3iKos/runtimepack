# Preserve all JavascriptInterface methods and their enclosing classes
-keepattributes JavascriptInterface
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
-keepclasseswithmembernames class * {
    @android.webkit.JavascriptInterface <methods>;
}

# Preserve Activities registered in AndroidManifest.xml
-keep public class * extends android.app.Activity

# Preserve native JNI methods (SDL, Ruby, mkxp-z, Ren'Py)
-keepclasseswithmembernames class * {
    native <methods>;
}

# Preserve RuntimeBuildInfo and RuntimeIntegrity constants & classes
-keep class io.runtimepack.host.RuntimeBuildInfo { *; }
-keep class io.runtimepack.host.RuntimeIntegrity { *; }
-keep class io.runtimepack.host.OverlayAssetSource { *; }

# Preserve Gpack models & AssetAudit serialization models
-keep class io.runtimepack.host.BuildManifestData** { *; }
-keep class io.runtimepack.host.RuntimeAudit** { *; }
