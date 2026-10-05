# gpui-storybook-automation

`gpui-storybook-automation` supplies target-neutral controls, presentation,
scenarios, interaction requests, snapshots, validation, and the asynchronous
`AutomationBackend` interface for automation hosts.

Hosts own their existing application runtime, thread dispatch, frame readiness,
and exclusive operation lifetime. Capability discovery controls MCP tool
exposure and rejects unsupported batches before dispatch. Clients preserve
partial-progress errors and never replay submitted mutations.

The gallery backend lives in `gpui-storybook-core`; Linux/macOS clients serve
it through `gpui-storybook-mcp`. The contract crate depends on Serde, Schemars,
JSON values, error derivation, and Bon builders and can be checked independently
for mobile targets. Its wire envelopes validate protocol/session identity and
bound JSON frames to 1 MiB. Host descriptors report native/GPUI agreement and
observed geometry; device capture records carry compositor provenance.
