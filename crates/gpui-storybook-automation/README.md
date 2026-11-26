# gpui-storybook-automation

`gpui-storybook-automation` supplies target-neutral controls, presentation,
scenarios, interaction requests, snapshots, validation, and the asynchronous
`AutomationBackend` interface for automation hosts.

Hosts own their existing application runtime, thread dispatch, frame readiness,
and exclusive operation lifetime. Capability discovery controls MCP tool
exposure and rejects unsupported batches before dispatch. Clients preserve
partial-progress errors and never replay submitted mutations. `WaitForValue`
steps gate later input on an exact rendered semantic value or JSON Pointer,
using a bounded frame limit. Final postconditions verify the completed flow.

The gallery backend lives in `gpui-storybook-core`; Linux/macOS clients serve
it through `gpui-storybook-mcp`.
