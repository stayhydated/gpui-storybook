# gpui-storybook-preferences

Internal preference boundary for GPUI Storybook. The crate owns validated
saved intent for appearance, independent light and dark themes, language, and
scrollbar behavior; one typed JSON document per consumer; generated JSON
Schema; and consumer-scoped repository CRUD.

## Saved intent and resolution

`StorybookPreferences` is durable user intent. It retains `System` or explicit
appearance and language selections, separate light and dark theme IDs, and the
scrollbar policy. `ResolvedPreferences` is the effective presentation after
device detection, supported-language negotiation, registered-theme
availability, and deterministic overrides. Its source enums and diagnostics
make every fallback inspectable without rewriting saved intent.

`System` appearance resolves from the current window scheme. `System` language
negotiates ordered device locales against the application's typed embedded
language set, then uses its configured typed fallback. Explicit choices ignore
later detections. Theme resolution reads only the slot for the effective
scheme, so a saved light-theme change remains dormant while dark appearance is
active and vice versa.

## JSON storage and schema

Every repository requires a validated, stable `ConsumerId`. The ID becomes the
default persistent filename and is embedded in the document so a file cannot
be loaded by a different Storybook binary accidentally. Applications give each
Storybook binary a distinct ID and keep it unchanged across launches.

`Persistent` repositories use `RepositoryOptions::json_path` when it is set or
an automatic `.gpui-storybook/{consumer-id}.json` path at the supplied Cargo
workspace or standalone package root. Facade consumers set the same override
with `StorybookOptions::with_json_path(...)`. The default directory contains a
generated `.gitignore` with `*`; an existing ignore file remains unchanged.
`Temporary` repositories own a unique JSON file for their lifetime, and
`Disabled` repositories retain only session memory. A path override is valid
only for `Persistent`.

The persisted document derives `serde::Serialize`, `serde::Deserialize`, and
`schemars::JsonSchema` from the same Rust types. Persistent repositories write a
shared sibling `preferences.schema.json` atomically and include its stable
filename in each document's `$schema` property. `preference_json_schema()` and
`preference_json_schema_pretty()` expose the same generated schema for tooling.
The schema publishes named `ConsumerId`, `ThemeId`, and `LanguageTag` definitions
with their validation constraints and examples; theme availability and the
supported locale set remain application-defined.
Preference writes use same-directory atomic replacement. Invalid persistent
JSON and consumer mismatches are archived with an injected timestamp suffix
before defaults are applied; ordinary filesystem failures remain errors and do
not move input files.

The crate also validates the ordered BCP 47 locales supplied by `sys-locale`,
provides an injected locale detector for deterministic tests, negotiates Fluent
languages against the consuming application's embedded set, resolves the GPUI
window appearance against injected theme-registry availability, and reports
typed fallback diagnostics for deterministic overrides.

Application developers use the `gpui-storybook` facade. This crate is an
implementation detail published to satisfy the facade and core crate dependency
graph. The facade exposes saved/resolved state, storage status, diagnostics,
readiness, retry behavior, and generated schema helpers without exposing the
repository itself.

Run the focused storage, detection, and resolution tests with:

```sh
cargo test -p gpui-storybook-preferences --locked
```
