# Portable device automation

This branch keeps the platform-neutral Storybook device protocol, bounded
transport, optional device coordinator, and ADB/MCP host. The earlier example
depended on an unofficial GPUI Android renderer. It is not part of the
Rust 1.100 workspace. Native GPUI mobile rendering and app qualification await
official upstream support.

## Use an application-owned endpoint

- Use `gpui-storybook-automation` for shared capabilities, semantic metadata,
  and versioned wire messages.
- Use `gpui-storybook-mobile` for a loopback endpoint, exclusive admission,
  operation settlement, and endpoint-owned session generations.
- On supported GPUI platforms, `gpui-storybook-automation-gpui` can coordinate
  an application's own native bridge when its `device` feature is enabled.
  The application owns all native integration, observation, and permission
  checks. Opt in explicitly instead of exposing an endpoint by default.

Surface replacement invalidates the previous session. Retain all leases until
the action has completed or the surface has been invalidated. Never replay a
submitted mutation or capture when the transport outcome is uncertain.

## Attach a computer-owned host

For a consumer that already implements the matching protocol on its app
endpoint, attach using an explicit device serial and app-owned
`storybook.toml`. No in-tree Android app or emulator image is provided.

```sh
cargo run -p gpui-storybook-mobile-host --locked -- \
  doctor --serial emulator-5580 --config storybook.toml
cargo run -p gpui-storybook-mobile-host --locked -- \
  serve --serial emulator-5580 --config storybook.toml
```

Interaction stays disabled until explicitly allowed. Display and named-surface
captures use observed device geometry and report ADB compositor provenance.
The host's MCP protocol uses stdio, and diagnostics use stderr.

## Verify the portable contracts

```sh
cargo test -p gpui-storybook-automation -p gpui-storybook-mobile --locked
cargo test -p gpui-storybook-mobile-host --locked
```

CI also checks the neutral contract crates on AArch64 Android, x86-64 Android,
and AArch64 iOS. These compile checks are not claims of running an Android or
iOS GPUI application.
