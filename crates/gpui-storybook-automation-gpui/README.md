# GPUI Storybook automation for GPUI

Attach automation to an application's existing root and window. Rendered-region,
semantic-target, and public-value instrumentation uses the application's real
GPUI state. Each app/window owns its registry; a new root frame removes stale
route metadata.

Implement `EmbeddedRoot` for route selection and public revision. Optional
controls, actions, presentation, and fresh fixtures have defaults. Application
availability uses the optional `readiness` hook. Register
compact `EmbeddedRoute` metadata and choose supported capabilities explicitly.

With `android`, `DeviceHost::attach_android` and the SDK's Kotlin
`StorybookAutomation` adapter own native lifecycle, geometry revisions, JNI
selection dispatch, committed-frame acknowledgment, attachment replacement, and
polling. `DeviceHostOptions` opens no endpoint by default; application build/runtime
opt-in enables it. Retain `NativeShellHandle` for native callbacks and run the
owner against the existing window. Display rotation replaces sessions; IME-only
viewport changes preserve them.

Declare named `HostAppearance` choices and override
`EmbeddedRoot::apply_host_appearance` for application schemes such as OLED.
The default supports standard light/dark IDs. Authentication, fixtures,
permissions, native actions, and their argument validation remain application-owned.

Lower-level integrations use `GpuiHostAttachment`, capture providers,
`AttachedInteraction`, and the `device` feature's `DeviceCoordinator`.
Whole-batch preflight precedes dispatch. Submitted work retains its lease through
native and GPUI frames after response cancellation. Native queues check retained
permits immediately before dispatch and distinguish known rejection from unknown
submission. Surface invalidation revokes deferred work without replay.

Native semantic-value keys are distinct from GPUI keys. The maintained
Counter/Notes example exercises the shared production root, SDK Android adapter,
and independent Activity-owned Compose state.
