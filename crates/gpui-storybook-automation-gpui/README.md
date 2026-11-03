# GPUI Storybook automation for GPUI

Attach automation to an application's existing GPUI root and window through
`GpuiHostAttachment`. Implement `EmbeddedRoot` for route selection, public
revision, readiness, controls, actions, presentation, and fresh fixtures.
Register compact `EmbeddedRoute` metadata and advertise supported capabilities.

Rendered regions, semantic targets, and public values use the application's
live state. Each window owns its registry; a new root frame clears stale route
metadata. Invalidate window regions before replacing a surface.

`AttachedInteraction` validates the entire batch before dispatch. Keystrokes
send both press and release events so native controls finish their activation.
Application authentication, fixtures, permissions, and action argument validation
remain with the application.

Enable `capture` for image capture through the host renderer. Capture providers
and the region registry support custom hosts using the same instrumentation.
