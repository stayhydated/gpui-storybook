plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.storybook.mobile"
    compileSdk = providers.gradleProperty("storybookPlatform").get().toInt()
    buildToolsVersion = providers.gradleProperty("storybookBuildTools").get()
    defaultConfig {
        applicationId = "dev.storybook.mobile"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = providers.gradleProperty("storybookVersion").get()
        ndk.abiFilters += providers.gradleProperty("storybookAbi").get()
    }
    sourceSets.getByName("main") {
        jniLibs.srcDir(providers.gradleProperty("storybookJni").get())
        assets.srcDir(providers.gradleProperty("storybookAssets").get())
    }
    buildFeatures.compose = true
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    packaging.jniLibs.useLegacyPackaging = true
}
kotlin.compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
dependencyLocking { lockAllConfigurations() }

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.12.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.11.0")
}
