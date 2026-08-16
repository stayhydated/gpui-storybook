# MCP Semantic Automation Implementation Plan

Status: complete (2026-08-15)

## Outcome

GPUI Storybook MCP sessions provide stable semantic interaction targets,
route-local structured value readback,
launch through one reusable Linux headless command, and terminate the GPUI
application when the stdio session ends. The Mitsubishi ZMQ client validates
the resulting workflow through a local path dependency before its portable Git
pin is updated.

## Public contracts

### Semantic targets

- `gpui-storybook-core` owns an element wrapper that records a stable target
  key, a human-readable label, and fresh story-relative bounds during
  prepaint.
- Target keys are unique within one story or substory route.
- `gpui-storybook` re-exports the wrapper as the application-facing API.
- The interaction-gated MCP surface lists targets for the active route.
- `storybook_run_steps` accepts a `click_target` step and resolves the target
  after route selection, viewport sizing, scrolling, and rendering.
- Target clicks use the same pointer move/down/up dispatch and partial-progress
  reporting as coordinate clicks.

### Structured semantic values

- `gpui-storybook-core` owns a second route-scoped prepaint registry for stable
  keys, human-readable labels, and JSON values sourced from application state.
- `gpui-storybook` exposes `semantic_value(key, label, value, child)` as the
  application-facing wrapper. Keys are unique within one story or substory
  route.
- The read-only `storybook_read_semantic_values` MCP tool requests a fresh
  frame and returns values for the active route in stable key order.
- Semantic reads can observe transitional asynchronous state and are safe to
  poll. They do not acquire the mutation guard or retry an interaction.
- Structured values establish state postconditions. Frame capture remains a
  separate surface for validating pixels, layout, and other visual behavior.

### Linux launcher

- A small publishable launcher crate starts Sway with the wlroots headless and
  Pixman software-rendering configuration, waits for its Wayland socket, runs
  the requested command with inherited stdio, and always cleans up the
  compositor and private runtime directory.
- The launcher accepts an explicit Sway executable so repository and CI
  workflows can use a privately provisioned binary.
- The MCP launch-environment tool and the standalone command share the same
  environment and readiness contract.

### Stdio lifecycle

- Starting the Storybook stdio server returns an awaitable completion signal.
- The facade observes server completion and quits the GPUI application after
  client EOF or a transport/server failure.
- Server failures remain visible on standard error; standard output stays
  reserved for JSON-RPC.

## Repository work

1. Add target registry and wrapper support beside capture-region tracking.
2. Extend core interaction request, preparation, resolution, dispatch, errors,
   snapshots, and focused tests.
3. Extend MCP tool registration, strict JSON schemas, structured results, and
   schema tests.
4. Add the headless launcher crate and process-level launcher tests.
5. Make the facade own stdio completion and application shutdown.
6. Update the inert interaction example to expose a semantic pointer target.
7. Synchronize root and crate READMEs, the automation book chapter, example
   READMEs, catalog copy, Rustdocs, and the public Storybook skill.
8. Execute one process-level Linux smoke workflow covering launch, MCP discovery,
   semantic target discovery, target clicking, and EOF shutdown.
9. Add route-scoped structured value registration and a read-only MCP tool,
   then extend the smoke workflow to establish its postcondition without frame
   capture.

## Downstream proof

1. In `tachyon-mitsubishi`, temporarily import the affected Storybook crates by
   local path for all implementation validation.
2. Forward the UI package's `mcp` feature to `gpui-storybook/mcp` and route
   MCP-session diagnostics to standard error.
3. Update the form generator to mark each generated execute button as the
   stable `execute-request` interaction target, then regenerate and check the
   owned form output.
4. Build and run `tm_zmq_server` on the Windows Mitsubishi VM.
5. Launch `tm-zmq-client-ui` on Linux with the headless launcher and Storybook
   MCP interaction enabled.
6. Discover `execute-request`, invoke it through `click_target`, and poll the
   generated route's `response` semantic value until it contains a real
   current-position response; verify the matching Windows server trace without
   requesting frame capture.
7. Close MCP stdin and verify the Linux GUI and compositor exit without an
   explicit quit action.
8. Restore portable Git dependencies, update the Storybook revision, and run a
   final locked dependency check.

## Acceptance evidence

- Focused core and MCP unit tests cover target uniqueness, target lookup,
  semantic-value uniqueness and ordering, strict schemas, coordinate
  resolution, and error reporting.
- Launcher tests cover argument parsing, environment construction, child exit
  propagation, and cleanup.
- The process smoke test proves real JSON-RPC stdio and EOF shutdown.
- `just fmt`, focused package tests, `just check`, `just clippy`, and affected
  docs builds pass in `gpui-storybook`.
- The Mitsubishi generator check and targeted Rust package checks pass on
  Linux.
- The Windows server build succeeds and the Linux Storybook returns a current
  position through semantic MCP interaction and structured readback without a
  screenshot.

## Completed validation

- `just fmt`, `just check`, `just clippy`, and `just test` passed in the
  Storybook workspace.
- `cargo xtask build book`, `cargo xtask build llms-txt`, and
  `cargo doc --workspace --all-features --no-deps --locked` passed.
- The launcher's process test proved child-status propagation plus Sway and
  temporary-runtime cleanup.
- The Mitsubishi form generator check and `tm-zmq-client-ui` MCP build passed
  on Linux while Storybook was imported by local path.
- The Windows VM built and ran `tm_zmq_server`; the Linux GUI opened
  `tm-zmq-client-ui-GetCurrentPositionForm`, discovered `execute-request`,
  clicked it semantically, and read the route-local `response` as
  `{ "status": "success", "value": { "Position": { "position": 0.0 } } }`.
  The workflow made no capture call. The server trace recorded the matching
  position request and successful response.
- Closing MCP stdin removed the GPUI windows and terminated the Linux GUI,
  launcher, and private Sway process with exit status 0.
