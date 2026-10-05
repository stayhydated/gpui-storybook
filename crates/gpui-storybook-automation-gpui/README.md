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
