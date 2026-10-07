# GPUI Storybook mobile host

Launch, diagnose, and verify an explicitly selected Android application's
embedded GPUI automation. Consumer `storybook.toml` metadata supplies the package,
Activity, port, APK, optional build hook, and runtime/fixture extras. The computer
owns direct ADB connections and PNG destinations; the application owns public
state, effects, and admission.

```sh
gpui-storybook-mobile-host serve --serial emulator-5580 \
  --config storybook.toml --launch --allow-interaction
gpui-storybook-mobile-host doctor --serial emulator-5580 --config storybook.toml
```

`smoke` runs application-owned semantic assertions, scoped PNG checks, and
explicit owned-emulator lifecycle qualification, producing a JSON report with
completed checks and typed failures. `android-source` exports the SDK's matching
Kotlin Activity/SurfaceView adapter.

The host uses bounded `adbutils-rs` connections and shell-v2 exit status.
Installation submits once and preserves the package on failure. Owned-process
cleanup compares PIDs and preserves replacements. Capture publication is atomic;
provider failures and caller cancellation retain ticket settlement, and
`SettlementFailed` preserves both typed operation and cleanup errors.

`RemoteBackend::attach_when_ready` repeats discovery and reports the final typed
observation on timeout. Submitted mutations and captures are single attempts.
Retain the initial runtime through `RemoteBackend::shutdown`,
`SmokeRunner::shutdown`, and `AdbTransport::shutdown` to settle admitted work.
`SmokeRunner` retains lifecycle restoration across canceled response futures.
Diagnostics use stderr; MCP uses stdin/stdout.

Protocol version 2 reports named appearances, observed geometry, capabilities,
and actionable readiness failures. Display and GPUI PNGs identify their
`adb_compositor_observation` provenance. Device captures use observed dimensions;
sizing and capture-control application require dedicated advertised providers.
