import java.util.Properties

plugins { id("com.android.application") }

val requestedAbis = providers.gradleProperty("gpuiAbis")
    .orElse(providers.gradleProperty("gpuiAbi"))
    .getOrElse("arm64-v8a")
require(!(providers.gradleProperty("gpuiAbis").isPresent && providers.gradleProperty("gpuiAbi").isPresent)) {
    "Use either gpuiAbi or gpuiAbis, not both."
}
val gpuiAbis = requestedAbis.split(',').map { value ->
    when (val abi = value.trim()) {
        "aarch64", "arm64-v8a", "aarch64-linux-android" -> "arm64-v8a"
        "x86_64", "x86_64-linux-android" -> "x86_64"
        else -> error("Unsupported Android ABI '$abi'. Use aarch64 (arm64-v8a) or x86_64.")
    }
}.distinct()

android {
    namespace = "dev.gpui.example"
    ndkPath = providers.environmentVariable("ANDROID_NDK_HOME").orNull
    ndkPath?.let { path ->
        val properties = Properties()
        file("$path/source.properties").inputStream().use { properties.load(it) }
        ndkVersion = properties.getProperty("Pkg.Revision")
    }
    compileSdk { version = release(36) { minorApiLevel = 1 } }
    defaultConfig {
        applicationId = "dev.gpui.example"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
        ndk { abiFilters += gpuiAbis }
    }
    sourceSets["main"].jniLibs.directories.add(layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath)
}

val rustBuilds = gpuiAbis.map { abi ->
    tasks.register<Exec>("buildRust_${abi.replace('-', '_')}") {
        environment("GPUI_ANDROID_ABI", abi)
        workingDir(rootProject.projectDir)
        commandLine("bash", "../build-rust.sh", layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath)
        inputs.property("abi", abi)
        inputs.files(fileTree("../../src"), fileTree("../../examples/hello_android/src"))
        // Cargo tracks the complete Rust dependency graph and incrementally rebuilds it.
    }
}
val buildRust by tasks.registering {
    dependsOn(rustBuilds)
}
tasks.named("preBuild").configure { dependsOn(buildRust) }

dependencies { implementation(project(":host")) }
