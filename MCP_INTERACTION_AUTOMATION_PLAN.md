# MCP Interaction Automation Plan

- **Status:** Proposed
- **Date:** 2026-08-06
- **Audience:** `gpui-storybook` maintainers and MCP integration authors

## Goal

Add opt-in, in-process interaction automation to a live GPUI Storybook window.
An MCP client should be able to focus controls, dispatch registered GPUI
actions, send keyboard input, move or click the pointer, wait for rendered
frames, and capture the resulting story without Gamescope, XTest, or another
operating-system input injector.

Typed story controls remain the preferred interface for reproducible inputs.
Pointer and keyboard automation cover behavior that cannot be represented by a
`ControlValue`, including focus, open popovers, keyboard navigation, pressed
states, and application actions.

This proposal does not make arbitrary application elements discoverable by
`ElementId`. The initial coordinate contract is bounded to the current story or
substory capture region. Stable element locators require a separate public GPUI
inspection contract and are deferred.

## Current Baseline

This plan builds on the work in [Storybook Workbench
Plan](STORYBOOK_WORKBENCH_PLAN.md), including its current controls and
automation vertical slice.

The current implementation provides:

- A `StorybookAutomationCommand` channel that moves MCP requests onto the live
  GPUI window thread in both gallery and dock modes.
- Native story capture through `Window::render_to_image`, followed by cropping
  to the registered story or substory region.
- Typed `ControlSpec` and `ControlValue` metadata plus control read, set, reset,
  and capture-time overrides.
- MCP tools for discovery, navigation, controls, viewport selection, and
  capture.
- A capture-region registry that records story-local bounds during prepaint.

The relevant implementation boundaries are
[`automation.rs`](crates/gpui-storybook-core/src/automation.rs),
[`capture_region.rs`](crates/gpui-storybook-core/src/capture_region.rs), the
gallery and dock command handlers, and the typed tool registry in
[`gpui-storybook-mcp`](crates/gpui-storybook-mcp/src/lib.rs).

The pinned GPUI revision already exposes the primitives needed by the proposed
executor:

- `Window::dispatch_keystroke`
- `Window::dispatch_event`
- `Window::dispatch_action`
- `Window::focus_next`, `Window::focus_prev`, and `Window::blur`
- `Window::resize`
- `Window::on_next_frame`
- `App::build_action`, `App::all_action_names`, and `App::action_schemas`

These are public APIs in the pinned
[`Window`](https://github.com/stayhydated/zed/blob/4a509a80b1452cb3da6edcfdac5c6b6c6fabf256/crates/gpui/src/window.rs)
and
[`App`](https://github.com/stayhydated/zed/blob/4a509a80b1452cb3da6edcfdac5c6b6c6fabf256/crates/gpui/src/app.rs)
implementations. The missing layer is Storybook-owned validation, sequencing,
and MCP schemas around those APIs.

## Design Decisions

### Use one bounded interaction batch

Add `storybook_run_steps` as the primary mutation tool. A request contains an
ordered list of typed steps and an optional capture request. Steps execute in
one UI-thread operation, so another MCP round trip cannot interleave between a
click and the capture intended to observe it.

Add `storybook_list_actions` as a read-only discovery tool. It returns the
non-internal registered GPUI action names, documentation, and JSON argument
schemas needed by `dispatch_action` steps.

Do not initially add separate MCP tools for every input primitive. The public
core types can still expose convenience builders, but one MCP batch keeps tool
discovery small and gives ordering, limits, cancellation, and safety one
contract.

### Execute through the existing automation host

Extend `StorybookAutomationCommand` with one long-running operation:

```text
MCP task
  -> StorybookAutomation::run_steps
  -> StorybookAutomationCommand::RunSteps
  -> gallery or dock host on the GPUI window thread
  -> frame-aware interaction executor
  -> optional native story capture
  -> typed result over the existing oneshot response
```

Gallery and dock mode must share the executor. Their handlers may prepare the
active story and workbench state, but pointer translation, action construction,
input dispatch, frame waits, capture, and result construction belong in
`gpui-storybook-core::automation` so the modes cannot drift.

### Prefer semantic controls, then actions, then input

Automation should use the highest-level stable contract that can express the
operation:

1. Use `storybook_set_control` for declared story inputs.
2. Use a registered GPUI action when the focused component exposes one.
3. Use keystrokes for keyboard-specific behavior.
4. Use story-relative pointer coordinates as the final fallback.

The executor must not translate control setters into synthetic clicks. The
existing `ControlTarget` path supplies validation, typed values, and structured
post-state that pointer events cannot guarantee.

### Bound pointer coordinates to the capture region

Initial pointer steps target the active route's capture region rather than the
entire Storybook window. This keeps automation away from the gallery sidebar,
workbench, title bar, and other Storybook chrome.

Support two story-relative coordinate spaces:

| Space | Contract | Use |
|---|---|---|
| `normalized` | `x` and `y` are in `0.0..=1.0` across the current route bounds | Preferred for scale-independent automation |
| `logical_pixels` | `x` and `y` are GPUI logical pixels from the route origin | Precise integration tests and known layouts |

`normalized` is the default. The executor resolves the latest
`CaptureRegionBounds`, converts the point to window coordinates, and rejects
non-finite or out-of-range values. It must resolve bounds after any resize and
after the route has rendered; cached coordinates from an earlier frame are not
valid.

Arbitrary window coordinates, global screen coordinates, and selector-based
clicks are out of scope for the first implementation. GPUI keeps its
rendered-frame element state, hitbox registry, and debug bounds behind
crate-private or test-oriented APIs; stable locators require a separate public
GPUI contract.

### Report dispatch, not semantic success

GPUI can report whether a keystroke or platform event propagated, but generic
input dispatch cannot prove that an application operation succeeded.
`storybook_run_steps` must report that a step was validated and dispatched; it
must not claim that a button completed its business action.

Callers establish postconditions with one or more of:

- the optional next-frame capture,
- `storybook_read_controls`,
- `storybook_current_story`, or
- a future story-specific state probe.

## Core Interaction Model

Add serializable, MCP-independent types to `gpui-storybook-core`:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoryInteractionStep {
    FocusNext,
    FocusPrevious,
    Blur,
    Keystrokes { keys: Vec<String> },
    Text { value: String },
    DispatchAction { name: String, args: Option<serde_json::Value> },
    PointerMove { point: StoryPoint },
    PointerClick {
        point: StoryPoint,
        button: StoryMouseButton,
        click_count: u8,
        modifiers: StoryModifiers,
    },
    Scroll {
        point: StoryPoint,
        delta_x: f32,
        delta_y: f32,
    },
    WaitFrames { count: u16 },
}
```

The exact Rust spelling can change during implementation, but the serialized
shape must remain closed, tagged, and representable by the MCP schema builder.

`Keystrokes` uses `Keystroke::parse`, preserving GPUI's existing binding
syntax. `Text` is a distinct operation for focused text input; it must not
pretend to implement platform input method editor composition, clipboard
paste, or dead-key behavior. The feasibility spike must verify plain ASCII and
Unicode insertion before this variant becomes public.

`PointerClick` dispatches move, down, and up events at the same resolved point.
This preserves hover and focus behavior that a bare mouse-up event would miss.
Drag, touch, pinch, file-drop, and native clipboard automation remain deferred
until a concrete story requires them.

### Request and result

`StoryInteractionRequest` should contain:

- An optional route key to open before executing.
- Optional typed control overrides applied before interaction.
- Optional paired dimensions or a named viewport applied before interaction.
- A required non-empty step list.
- An optional capture output path.
- Explicit safety limits or defaults for steps, text size, and waited frames.

The optional capture means “capture the first rendered frame after the final
step,” except that explicit `WaitFrames` steps delay that point. The executor
must not reuse the existing two-MCP-call sequence for this case.

`StoryInteractionSnapshot` should return:

- A request ID and active `StorySnapshot`.
- The number of steps validated and dispatched.
- Per-step dispatch observations where GPUI provides them.
- Whether any focus handle exists after the batch; no unstable focus ID should
  be serialized.
- The optional `StoryCaptureSnapshot`.

Action dispatch is deferred by GPUI and returns no handler result. Its step
observation should therefore be `dispatched`, not `handled` or `succeeded`.

## Frame Ordering and Exclusivity

Frame ordering is the central correctness requirement.

1. Resolve and optionally open the route.
2. Apply controls and the requested viewport or dimensions.
3. Scroll the target capture region into view.
4. Request a refresh and wait for its rendered frame.
5. Resolve fresh route bounds.
6. Execute synchronous steps in order.
7. For `WaitFrames`, refresh and continue through `Window::on_next_frame`.
8. If capture was requested, refresh and capture the first rendered frame
   after the final step.
9. Complete the oneshot response and release the operation guard.

The implementation should use a small owned state machine scheduled with
`Window::on_next_frame`. After host preparation, the state machine should not
need mutable access to gallery- or dock-specific state.

Only one capture or interaction batch may be active. Replace or generalize the
current `capture_pending` flag with an exclusive-operation guard shared by
`capture_current_story` and `run_steps`. The guard must clear through `Drop` so
window closure, response cancellation, validation failure, or capture failure
cannot leave automation permanently busy.

While a batch is pending:

- Reject other capture or mutation requests with a structured
  `AutomationBusy` error.
- Permit catalog and current-story reads.
- Document that reads can observe an intermediate rendered state.
- Do not silently queue mutations whose eventual execution point would be
  ambiguous to the caller.

Bound a batch to at most 64 steps, 4 KiB of text, 120 waited frames, and one
capture initially. Reject zero-frame waits and non-finite coordinates or
scroll deltas. These are validation limits, not promises that every allowed
batch will complete within a fixed wall-clock duration.

## MCP Surface

### `storybook_list_actions`

Return action data from `App::action_schemas` and matching documentation. Only
non-internal registered actions should be advertised. The output must use a
closed schema and remain read-only.

Action names are runtime registrations, not durable Storybook route IDs. The
tool description must tell clients to rediscover them for each launched
application.

### `storybook_run_steps`

Register a typed asynchronous tool that forwards to
`StorybookAutomation::run_steps`. Its input schema should use a tagged `oneOf`
for steps and reject unknown fields.

Example shape:

```json
{
  "steps": [
    {
      "type": "pointer_click",
      "point": { "space": "normalized", "x": 0.35, "y": 0.45 },
      "button": "left",
      "click_count": 1,
      "modifiers": []
    },
    { "type": "keystrokes", "keys": ["down", "enter"] },
    { "type": "wait_frames", "count": 1 }
  ],
  "capture": {
    "output_path": "target/storybook-captures/interaction.png"
  }
}
```

The tool is mutating, non-idempotent, potentially destructive, and open-world:
a story can bind a click or action to filesystem, network, hardware, or process
effects. These MCP annotations are part of the contract, not documentation
decoration.

### Capability gate

Do not enable generic interaction merely because read-only story discovery is
enabled. Require an explicit runtime capability such as
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` before registering
`storybook_list_actions` and `storybook_run_steps`.

The environment variable is appropriate for stdio launches and preserves the
existing `mcp` feature as the build-time integration switch. Direct embedders
should also receive a typed server-options API so they do not need to mutate
process environment. When the capability is disabled, omit the tools from
discovery instead of registering tools that always fail.

The gate authorizes the input mechanism, not every downstream effect. Storybook
applications that can reach production services or hardware remain
responsible for launching against a safe backend.

## Implementation Phases

### Phase 0: Input and frame-ordering spike

Build an internal test story and prove the pinned GPUI behavior before adding
public types:

1. Dispatch a story-relative click and observe a counter change.
2. Focus an input and insert plain text with `dispatch_keystroke`.
3. Navigate a `gpui-component` select using `down` and `enter`.
4. Build and dispatch one registered action by name and JSON arguments.
5. Resize, resolve fresh capture bounds, click, and capture the next frame.
6. Verify that one-frame capture can observe a transient pressed or busy state.

If text injection cannot preserve the intended Unicode string through the
public keystroke API, omit the `Text` step initially and document the supported
`Keystrokes` path. Do not reach into GPUI's private platform input handler.

### Phase 1: Core types and executor

1. Add interaction request, step, point, modifier, result, and error types to
   `gpui-storybook-core::automation` or a focused sibling module.
2. Add `StorybookAutomationCommand::RunSteps` and
   `StorybookAutomation::run_steps`.
3. Implement closed input validation and action construction.
4. Translate story-relative coordinates through `capture_region_bounds`.
5. Add the frame-aware executor and shared exclusive-operation guard.
6. Reuse capture path generation, image rendering, cropping, and result types.
7. Attach the same preparation and executor entry point in gallery and dock
   mode.

Refactor shared capture scheduling only as far as needed to keep normal capture
and interaction capture on one rendering/cropping implementation.

### Phase 2: MCP tools and capability configuration

1. Add typed input and output schemas for action discovery and interaction
   steps.
2. Register `storybook_list_actions` and `storybook_run_steps` when the runtime
   capability is enabled.
3. Map every validation, busy, missing-route, missing-region, invalid-action,
   host-disconnect, and capture error to structured MCP errors.
4. Add server-options plumbing for direct embedders and environment parsing for
   stdio launches.
5. Test tool metadata, closed schemas, unknown fields, bounds, and capability
   gating.

### Phase 3: End-to-end interaction fixtures

Add deterministic example stories that exercise:

- click and hover state,
- keyboard focus and text input,
- select open/navigation/confirmation,
- a registered typed action,
- viewport resize before input,
- transient next-frame capture, and
- failure without an active or rendered route.

Cover gallery and dock hosts. The tests must use GPUI's in-process dispatch and
native capture paths; passing only through X11, Wayland, or macOS event
injection would not validate this design.

### Phase 4: Optional semantic state probes

Evaluate this only after the interaction executor is stable. Typed controls
describe story inputs, but they do not expose internal states such as
`command_in_flight`, a polling sequence, or a receipt.

A future object-safe `StoryAutomationProbe` could return a bounded,
serializable snapshot from the active concrete story instance. If adopted, it
should be opt-in per story and use a dedicated MCP read tool. Do not expose an
arbitrary entity graph or allow reflection over application state.

## Failure Behavior

| Failure | Required result |
|---|---|
| No live gallery or dock host | Return `NoLiveHost` without dispatching steps |
| Another capture or batch is active | Return `AutomationBusy` without queuing mutation |
| Route is missing or not rendered | Return a structured route or capture-region error |
| Coordinate is invalid or outside its space | Reject the complete batch before input dispatch |
| Keystroke cannot be parsed | Identify the step index and invalid value |
| Action is unknown or arguments fail its schema | Identify the step index and action; dispatch nothing |
| Window closes during a frame wait | Drop the runner, clear the operation guard, and return host disconnect |
| Capture fails after steps were dispatched | Report partial execution plus capture failure; do not retry input |
| MCP caller cancels | Stop at the next safe executor boundary when detectable; always clear the guard |

Validate the complete request, including every keystroke and action, before
dispatching the first step. Runtime failures after dispatch are partial success
and must carry the number of already dispatched steps. Automatic retries are
unsafe because clicks and actions are not generally idempotent.

## Security and Trust Boundary

The MCP server is a process-local control plane for the application, not a
sandbox. Once interaction is enabled, a client can activate any behavior
reachable from the current story and focused action context.

The implementation must:

- keep generic pointer coordinates inside the active story capture region,
- require explicit interaction enablement,
- annotate the mutation tool as potentially destructive and open-world,
- reject unbounded step, text, and frame-wait inputs,
- avoid file-drop, clipboard, shell, and native drag primitives initially, and
- never describe event dispatch as authorization for the action it triggers.

Tests and documentation examples must use inert local stories. Hardware,
production network, credential, and destructive filesystem behavior are not
acceptable automation fixtures.

## Acceptance Criteria

- An MCP client can open a story, apply typed controls, focus a widget, operate
  a select, edit a basic text or number input, click a button, wait for frames,
  and capture the result without external input injection.
- A click followed by optional capture is one exclusive UI-thread operation;
  another MCP mutation cannot interleave.
- The capture is the first requested rendered frame after the final step or
  explicit wait, making short-lived visual states testable.
- Pointer coordinates remain correct after a viewport resize and across display
  scale factors.
- Gallery and dock modes expose identical behavior and structured failures.
- Invalid batches dispatch no input. Runtime partial failures report the number
  of dispatched steps and are never retried automatically.
- Action discovery lists runtime-visible actions and action dispatch validates
  names and JSON arguments before execution.
- Interaction tools are absent unless explicitly enabled.
- Navigation, typed-control, startup-capture, and live-capture paths share the
  exclusive-operation rules and pass their focused tests.

## Validation

Run focused checks as the phases land:

```bash
cargo test -p gpui-storybook-core --all-features --locked
cargo test -p gpui-storybook-mcp --all-features --locked
cargo check --manifest-path examples/story/Cargo.toml --features dock,mcp --locked
cargo check --manifest-path examples/component/Cargo.toml --features dock,mcp --locked
just fmt
just clippy
just test
cargo xtask build book
cargo xtask build llms-txt
```

Add schema tests for both MCP tools and GPUI-context integration tests for the
executor. Manually run one stdio MCP session with the interaction gate enabled
and verify the capture workflow on the supported native platforms available to
the maintainer. Record unavailable platform coverage rather than substituting
compositor-level injection as proof of in-process behavior.

## Documentation Synchronization

When implementation lands, update all public MCP workflow surfaces required by
`AGENTS.md`:

- `README.md`
- `crates/gpui-storybook/README.md`
- `crates/gpui-storybook-core/README.md` and automation Rustdocs
- `crates/gpui-storybook-mcp/README.md` and crate Rustdocs
- `book/src/automation.md`
- `skills/use-gpui-storybook/SKILL.md`
- `skills/use-gpui-storybook/references/automation-and-capture.md`
- affected example READMEs and stories
- matching catalog copy in `web/src/lib.rs`

Keep maintainer-only executor details in Rustdocs, source comments, tests, and
this plan. User-facing documentation should lead with capability enablement,
safe launch configuration, tool schemas, and examples.

## Deferred Work

- Stable element locators and bounds lookup by semantic ID.
- Accessibility-tree discovery and action invocation.
- Window-chrome or global-screen coordinate input.
- Drag, touch, pinch, pressure, file-drop, clipboard, and full input method
  editor simulation.
- Multiple captures inside one batch.
- Automatic waits for application-specific conditions.
- Generic reflection over entity or component state.

Each deferred capability changes the trust boundary or requires a stronger
public GPUI contract. It should be proposed and tested independently instead
of being hidden inside the initial input executor.
