# GPUI Storybook mobile host

Attach the Storybook MCP server to an explicitly selected Android device.
The computer owns its ADB forwarding and PNG destinations; the opted-in app
owns command admission and its retained runtime.

```sh
cargo run -p gpui-storybook-mobile-host -- \
  --serial emulator-5580 --allow-interaction
```

Use `--install <apk>` to install the repository example.
`--launch-example` starts a fresh example process with runtime automation enabled.
`--stop-on-eof` explicitly stops a launched example. MCP EOF releases the owned
forwarding. Device display and GPUI-region PNGs report ADB compositor provenance
and validate the session, route revision, and observed surface geometry.

The v1 ADB provider negotiates fixed observed geometry and standalone compositor
captures. Sizing, capture-control application, final interaction captures, and
desktop launcher capabilities require dedicated providers and fail negotiation
before mutation. Scenarios and steps use the existing GPUI frame executor.
