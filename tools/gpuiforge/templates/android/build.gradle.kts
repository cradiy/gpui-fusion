import java.util.Properties

plugins {
    id("com.android.library") version "9.2.1" apply false
    id("com.android.application") version "9.2.1" apply false
}

val localProperties = Properties().apply {
    val source = rootProject.file("local.properties")
    if (source.isFile) source.inputStream().use { load(it) }
}
val sdkPath = localProperties.getProperty("sdk.dir")
    ?: providers.environmentVariable("ANDROID_HOME").orNull
    ?: providers.environmentVariable("ANDROID_SDK_ROOT").orNull
    ?: error("Set ANDROID_HOME to an Android SDK installation")
val sdkVersion = file(sdkPath).resolve("platforms").listFiles().orEmpty()
    .mapNotNull { platform ->
        val match = Regex("android-([0-9]+)(?:\\.([0-9]+))?").matchEntire(platform.name)
            ?: return@mapNotNull null
        val major = match.groupValues[1].toIntOrNull() ?: return@mapNotNull null
        val minor = match.groupValues[2].ifEmpty { "0" }.toIntOrNull()
            ?: return@mapNotNull null
        if ((major > 36 || major == 36 && minor >= 1) && platform.resolve("android.jar").isFile)
            major to minor else null
    }
    .minWithOrNull(compareBy<Pair<Int, Int>> { it.first }.thenBy { it.second })
    ?: error("Install Android SDK platform 36.1 or later")
extra["gpuiCompileSdkMajor"] = sdkVersion.first
extra["gpuiCompileSdkMinor"] = sdkVersion.second
