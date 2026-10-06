plugins { id("com.android.library") }

android {
    namespace = "dev.gpui.android"
    compileSdk { version = release(36) { minorApiLevel = 1 } }
    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }
}
