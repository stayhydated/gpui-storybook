# Working in gpui-storybook

Start with `crates/gpui-storybook` for application-facing changes and
`just --list` for workspace commands. The executable examples show the two
registration styles: `examples/story` uses `#[story]` with `Story`, while
`examples/component` uses `#[derive(ComponentStory)]`. Use the pinned toolchain
in `rust-toolchain.toml` for local validation.

## Where changes belong

| Surface | Audience and responsibility |
|---|---|
| `crates/gpui-storybook` | Application facade: initialization, discovery, filtering, and public re-exports |
| `crates/gpui-storybook-core` | Runtime integration: gallery, workbench, story containers, preferences UI, and automation |
| `crates/gpui-storybook-automation` | Target-neutral automation contracts, validation, backend interface, and device wire protocol |
| `crates/gpui-storybook-automation-gpui` | Embedded GPUI instrumentation, frame execution, and native device coordination |
| `crates/gpui-storybook-mobile` | Opt-in device endpoint, session generations, operation admission, and transport shutdown |
| `crates/gpui-storybook-mobile-host` | Computer-owned ADB transport, APK installation, remote MCP, and atomic PNG capture |
| `crates/gpui-storybook-macros` | Public macro syntax and generated registrations, controls, and substory keys |
| `crates/gpui-storybook-toml` | Public configuration schema, loading, and filters |
| `crates/gpui-storybook-mcp` | Linux/macOS MCP tools, stdio serving, and capture launch helpers |
| `crates/gpui-storybook-launch` | Standalone Linux command that owns the headless Sway lifecycle |
| `crates/gpui-storybook-test` | Public test integration: fresh story contexts, captures, matrices, baselines, and frame budgets |
| `crates/gpui-storybook-preferences` | Internal typed persistence, system detection, and preference resolution |
| `examples/story`, `examples/component` | Executable application examples and registration fixtures |
| `examples/embedded`, `examples/mobile` | Shared production views, Android native shell, and maintained device qualification |
| `examples/mobile/native` | Hash-pinned AndroidX native input and lifecycle harness |
| `examples/mobile/android` | Pinned Gradle/Kotlin/Compose shell, dependency locks, and artifact verification |
| `examples/mobile/tools` | Isolated, unpublished Android library qualification probes |
| `book/src` | Application user guide; navigate through `SUMMARY.md` |
| `skills/use-gpui-storybook` | Application integration guidance for coding agents |
| `web/src/lib.rs` | Public catalog and site routes |
| `xtask` | Book, LLM text, demo, and site build orchestration |

Keep API contracts in Rustdocs and implementation details beside the source,
tests, snapshots, or fixtures that establish them. The book and integration
skill serve application developers.

## Keep related surfaces aligned

Update the public descriptions that cover changed behavior: root and facade
READMEs, the affected crate README, matching book sections, examples, and the
integration skill's relevant reference. Update `web/src/lib.rs` when its catalog
copy describes that behavior.

Keep READMEs focused on purpose, usage, and relevant constraints. Use CI,
Codecov, book, and crates.io badges for their destinations; leave installation
instructions and book or API navigation to those linked surfaces.

- **Registration and controls:** keep macro Rustdocs,
  `crates/gpui-storybook-macros/src/tests.rs`, and its `src/snapshots/` aligned.
  Update both example styles when a shared registration concept changes.
- **Duplicate keys:** keep
  `crates/gpui-storybook/tests/duplicate_story_key.rs` aligned with
  `crates/gpui-storybook/tests/fixtures/duplicate-story-key`.
- **Configuration:** synchronize TOML field semantics and runtime selection with
  both example `storybook.toml` files. `disable_story` matches registered type
  names; display titles and route keys are separate identities.
- **Runtime behavior:** update the owning core Rustdocs and runtime tests.
- **Preference storage:** preserve serialization of admitted disk mutations and
  cache updates across caller cancellation. Keep repository Rustdocs and
  `crates/gpui-storybook-preferences/src/tests/cancellation.rs` aligned with this
  contract.
- **Automation and capture:** keep MCP schemas, core automation/capture
  contracts, examples, and the automation skill reference aligned.
- **Android automation:** keep neutral wire contracts, device admission, native
  acknowledgment, owned host cleanup, and capture provenance aligned. Preserve
  caller-cancellation settlement, fresh endpoint-owned sessions, and typed
  secondary cleanup errors. Keep the example's build/runtime opt-in explicit.
  The mobile CI workflow gates release; library probes stay in their separate
  unpublished workspace.
  Compose state belongs to the Activity; native actions validate before enqueueing
  and acknowledge committed frames. Keep accessibility test tags, native value
  keys, MCP descriptors, and independent native/GPUI qualification aligned.
  Refresh Android dependencies deliberately with `build.py --write-gradle-locks`
  and review both Gradle locks and SHA-256 verification metadata.
  The Kotlin native harness shares the app's Gradle/compiler pins and verifies
  its AndroidX closure independently. Its builder owns a separate Gradle lockfile.
  Preserve JVM descriptors when editing Kotlin JNI callbacks.
- **Portable tests:** keep the test crate README, portable-testing and automation
  book sections, examples, and automation skill reference aligned when capture
  matrices, baseline policy, context setup, or frame budgets change.
  Pure property tests live in the test crate's `src/properties.rs` and
  `src/baseline/properties.rs`. Keep buffers and matrix products small, preserve
  valid unique axis labels while shrinking, and use independent decoding and
  known-delta metric oracles. Matrix cases sort by encoded ID; arbitrary
  baseline IDs and control digests do not promise universal injectivity.
- **Localization:** keep Rust locale code, core and example `i18n.toml` files,
  affected FTL catalogs, and locale setup instructions aligned when message keys
  or locale wiring change.

Build publication artifacts through `cargo xtask`: the sources are `book/src`,
`web/src`, and `examples/story`. The generated book, LLM text, demo, and site
outputs are not independent editing surfaces.

## Validate capture backends

The `master` branch uses published GPUI packages. Keep the Zed fork patches
and Wayland startup-capture regression on `wayland-render-image`. On that
branch, `just wayland-capture-test` captures the button story at desktop,
tablet, mobile, and custom sizes in private Sway, decodes the PNGs, and verifies
dimensions, rendered content, and exclusion of workbench controls. It requires
Sway and a working Wgpu adapter.

The `wayland-render-image` branch's root `[patch.crates-io]` pins
`gpui-pre-linux`, `gpui-pre-wgpu`, and `gpui-pre-platform` to one Zed fork
revision, compatible with `gpui-pre =0.3.7`. Keep those revisions and the patch
example in `book/src/automation.md` aligned, and commit the updated `Cargo.lock`
when changing a pin. Consumer applications apply these patches in their own
workspace root for Wayland window capture; Cargo patches are not inherited from
dependencies. Portable runner capture uses the published headless renderers.

## Validate the changed surface

Choose the narrowest relevant check and report its result, including any
unexecuted or failed checks.

| Change | Validation |
|---|---|
| Markdown | `rumdl check` with the edited paths |
| Book or LLM text | `cargo xtask build book` and `cargo xtask build llms-txt` |
| Macro expansion | `cargo test -p gpui-storybook-macros --locked` |
| Device wire or admission | `cargo test -p gpui-storybook-automation -p gpui-storybook-mobile --locked` |
| ADB transport, installation, or capture | `cargo test -p gpui-storybook-mobile-host --all-features --locked` |
| Embedded device coordinator | `cargo test -p gpui-storybook-automation-gpui -p gpui-storybook-example-embedded --all-features --locked` |
| Preference storage or resolution | `cargo test -p gpui-storybook-preferences --locked` |
| Portable runner | `cargo test -p gpui-storybook-test --all-features --locked` |
| Public Rust API docs | `cargo doc --workspace --all-features --no-deps --locked` on Linux |
| GPUI demo | `cargo xtask build gpui-demo` |
| Catalog | `cargo xtask build web` |

`just fmt` formats Rust, TOML, and Markdown. `just check` and `just clippy`
exclude the two example packages; `just test` includes them. `just web-build`
assembles all publication artifacts.

CI tests all features on Linux. On macOS it excludes the Linux-only launcher;
on Windows it uses default features and excludes the launcher, MCP crate, and
mobile host.
Use the matching platform scope when reproducing those jobs.

`just mobile-build [abi]` builds the opted-in Android example using
`ANDROID_HOME` and `ANDROID_NDK_HOME`. `just mobile-host <serial> [abi]`
installs/launches it and serves MCP. `just mobile-doctor <serial>` reads endpoint readiness and
`just mobile-smoke <serial>` runs the declared semantic plan and owned-emulator
lifecycle checks. `just mobile-test <serial>` qualifies an
already running example on an exclusively owned API 36 emulator, including
AndroidX native input and lifecycle. Preserve the selected device and forwarding
ownership boundaries; record renderer, image, font, and IME inputs with captures.
