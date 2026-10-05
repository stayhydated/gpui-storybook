# AndroidX native qualification

A separate test APK exercises the embedded example through AndroidX UI Automator
2.4.0. It selects Compose tabs and counter controls through resource-ID test
tags, verifies native clicks and MCP actions against rendered Compose text,
and checks independent native counter retention through lifecycle changes.
It taps GPUI targets using observed surface geometry,
commits `a` followed by a space through the actual AOSP keyboard, rotates and pauses/resumes the
Activity, dumps the accessibility hierarchy, and decodes a native screenshot
under a device capture ticket. GPUI targets come from Storybook's rendered registry.

```sh
python3 examples/mobile/native/build.py --sdk "$ANDROID_HOME"
python3 examples/mobile/native/verify.py --serial emulator-5580
```

Use an exclusively owned disposable Pixel 7 API 36 default x86_64 emulator,
portrait 1080 × 2400 display, AOSP en-US LatinIME, SwiftShader Vulkan, and the
opted-in example APK. The harness starts a fresh example process and restores
portrait rotation. Other keyboard layouts and native permission/effect flows
need their own qualification cases.
The qualified AOSP key-row centers are measured from the observed keyboard
bottom in density-independent pixels, allowing its suggestion strip to change
height.

The Kotlin harness uses the application's Gradle 8.13 wrapper, AGP 8.13.2,
Kotlin 2.3.10 compiler, JVM 17 target, JDK 21, and Android platform
36/build-tools 36.0.0. The builder verifies the AndroidX Maven closure in
`androidx-dependencies.json`; Gradle locks and SHA-256 metadata verify its Kotlin
runtime. Its annotations dependency comes from the independently pinned closure.
Refresh the Kotlin locks deliberately with `build.py --write-gradle-locks`.
The builder signs `target/mobile-native-apk/androidx.apk`.
The runner explicitly selects one emulator and stops its test package after
completion. Reports, instrumentation output, XML, and PNG live under
`target/mobile-evidence/androidx`. The mobile CI workflow builds and runs this
harness and preserves those artifacts alongside the MCP/native lifecycle proof.
