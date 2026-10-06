import java.util.Properties

plugins { id("com.android.application") }

val gpuiAbi = providers.gradleProperty("gpuiAbi").getOrElse("arm64-v8a")

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
        ndk { abiFilters += gpuiAbi }
    }
    sourceSets["main"].jniLibs.directories.add(layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath)
}

val buildRust by tasks.registering(Exec::class) {
    environment("GPUI_ANDROID_ABI", gpuiAbi)
    workingDir(rootProject.projectDir)
    commandLine("bash", "../build-rust.sh", layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath)
    inputs.files(fileTree("../../src"), fileTree("../../examples/hello_android/src"))
    // Cargo tracks the complete Rust dependency graph and incrementally rebuilds it.
}
tasks.named("preBuild").configure { dependsOn(buildRust) }

dependencies { implementation(project(":host")) }
