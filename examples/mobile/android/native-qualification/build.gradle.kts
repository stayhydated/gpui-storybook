plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.storybook.mobile.test"
    compileSdk = 36
    buildToolsVersion = "36.0.0"
    defaultConfig {
        applicationId = "dev.storybook.mobile.test"
        minSdk = 31
        targetSdk = 36
    }
    sourceSets.getByName("main") {
        manifest.srcFile("../../native/androidx/AndroidManifest.xml")
        java.srcDir("../../native/androidx")
        assets.srcDir(providers.gradleProperty("storybookNativeAssets").get())
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
kotlin.compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
dependencyLocking { lockAllConfigurations() }

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-stdlib:2.3.10") {
        // The independently verified AndroidX closure supplies annotations 23.0.0.
        exclude(group = "org.jetbrains", module = "annotations")
    }
    // The Python builder verifies the independently pinned AndroidX closure.
    implementation(fileTree(providers.gradleProperty("storybookNativeJars").get()) { include("*.jar") })
}
