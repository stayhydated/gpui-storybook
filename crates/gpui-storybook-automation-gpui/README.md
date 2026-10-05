# GPUI Storybook automation for GPUI

Rendered-region, target, and semantic-value instrumentation for an application-owned
GPUI window. Applications provide initialization and assets. Each app/window owns
its registry; a new root frame removes stale route metadata.

Implement `EmbeddedRoot` for route selection, typed controls, scoped actions,
presentation, public state revision, and fresh fixtures. Attach a catalog and
capture provider with `GpuiHostAttachment::attach`. The application owns command
admission and supplies a lease with `AttachedInteraction::builder()`.

Whole-batch preflight precedes dispatch. The executor retains the lease through
frame callbacks after response cancellation. Invalidate the attachment before
surface replacement; deferred input then stops against the stale root.
The Counter/Notes example renders and exercises this API with real GPUI contexts.

Enable `device` to use `device::DeviceCoordinator` with your `EmbeddedRoot`.
Supply an opted-in `DeviceEndpoint`, immutable `NativeShellSnapshot` observations,
and a native queue adapter. The coordinator owns preflight, native acknowledgment,
rendered-frame waits, capture tickets, and operation settlement. The adapter
retains `NativeSelection` and checks its permit and surface revision immediately
before dispatch on the native lifecycle owner thread, then acknowledges applied
or revoked work. Distinguish rejection before enqueueing from unknown delivery.
A response deadline retains ownership until acknowledgment or host invalidation.

Advertise application-native actions and semantic values in `NativeShellSnapshot`.
`HostAction::Invoke` reaches the native adapter as `NativeSelection::action()`;
validate its arguments before enqueueing and acknowledge the committed native
frame. Native value keys belong to the application and must be distinct from
GPUI keys. The Android example uses this boundary for its Jetpack Compose counter.

On native surface release, call `OperationGate::suspend` immediately. This closes
admission during the handoff to the GPUI owner. After invalidating the old
attachment, `DeviceCoordinator::surface_replaced()` advances the endpoint-owned
session generation and reopens admission atomically. Native surface revisions
remain observation metadata; each replacement receives a fresh session.
