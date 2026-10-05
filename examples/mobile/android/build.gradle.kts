plugins {
    id("com.android.application") version "8.13.2" apply false
    id("org.jetbrains.kotlin.android") version "2.3.10" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.10" apply false
}

allprojects {
    layout.buildDirectory.set(file(providers.gradleProperty("storybookBuildRoot").get()).resolve(name))
}
