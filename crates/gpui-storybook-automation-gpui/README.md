# GPUI Storybook automation for GPUI

Attach automation to an application's existing root and window. Rendered-region,
semantic-target, and public-value instrumentation uses the application's real
GPUI state. Each app/window owns its registry; a new root frame removes stale
route metadata.

Implement `EmbeddedRoot` for route selection and public revision. Optional
controls, actions, presentation, and fresh fixtures have defaults. Application
availability uses the optional `readiness` hook. Register
compact `EmbeddedRoute` metadata and choose supported capabilities explicitly.

With `device`, an application supplies its own native observation, acknowledgment,
and dispatch bridge to `DeviceHost`. No endpoint is opened by default. Native
GPUI mobile rendering awaits official upstream support; this crate does not
supply an Android renderer or JNI attachment.

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

Native semantic-value keys are distinct from GPUI keys. The Counter/Notes
example exercises the application-owned root and portable coordination on the
desktop.
