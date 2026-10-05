# GPUI Storybook mobile host

The maintained Android MCP host attaches to an explicitly selected device.
The computer owns direct ADB connections and PNG destinations; the opted-in app
owns command admission and its retained runtime.

```sh
cargo run -p gpui-storybook-mobile-host -- \
  --serial emulator-5580 --allow-interaction
```

The host uses selected `adbutils-rs =0.1.0` connection primitives with an
explicit serial and loopback ADB server (`--adb-server`, default 127.0.0.1:5037).
Host-owned budgets bound wire replies to 1 MiB, combined shell output to 1 MiB,
and encoded/decoded PNGs to 64 MiB. Shell-v2 preserves stdout, stderr, and exit
status. Submitted mutations are never replayed. ADB/platform tools provide the
local server; the SDK can start it before command submission.

Use `--install <apk>` to stream one APK of at most 512 MiB to a unique device
temporary path, install once, and remove that path on success or failure.
Installation failures preserve the installed package.
The 120-second install deadline includes local input validation, opening, upload,
and package installation. Special files are rejected before opening.
`--launch-example` starts a fresh example process with runtime automation enabled.
`--stop-on-eof` explicitly stops a launched example. MCP EOF settles admitted captures before host cleanup. Startup failure cleans up a newly launched example; cleanup checks its PID
before stopping it. Library users retain the attachment runtime and call
`RemoteBackend::shutdown` after closing admission and `AdbTransport::shutdown`
to settle admitted installs. Caller cancellation retains
capture ownership and final ticket settlement, including provider/decoder errors.
Omitted capture paths use a host-generated filename under `target/mobile-captures`.
Device display and GPUI-region PNGs report ADB compositor provenance
and validate the session, route revision, and observed surface geometry.
PNGs replace their destination atomically after successful encoding and ticket
settlement. Failed encoding preserves an existing artifact. If an operation and
its cleanup both fail, `SettlementFailed` retains both typed errors.
Set `RUST_LOG=gpui_storybook_mobile_host=debug` to observe request/session spans;
the command writes diagnostics to stderr.

The v1 ADB provider negotiates fixed observed geometry and standalone compositor
captures. Sizing, capture-control application, final interaction captures, and
desktop launcher capabilities require dedicated providers and fail negotiation
before mutation. Scenarios and steps use the existing GPUI frame executor.
