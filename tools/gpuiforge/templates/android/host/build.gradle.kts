plugins { id("com.android.library") }

android {
    namespace = "dev.gpui.android"
    compileSdk { version = release(36) { minorApiLevel = 1 } }
    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }
}

dependencies {
    implementation("androidx.core:core:1.18.0")
    implementation("androidx.media3:media3-exoplayer:1.9.0")
}
