# AGENTS.md

This is the working guide for contributors and coding agents in the
`gpui-storybook` workspace.

Use it to decide:

1. where to start,
2. whether a crate or surface is user-facing, public integration, or internal,
3. which docs, examples, skill guidance, tests, snapshots, and fixtures must
   change together,
4. which validation command should run before handoff.

For most application-facing code and docs, start with `crates/gpui-storybook`.

Reach for the example apps when you need to understand or demonstrate the two
supported registration styles:

- `examples/story` for explicit `#[story]` plus `Story` implementations.
- `examples/component` for `#[derive(ComponentStory)]` on the component itself.

`CLAUDE.md` delegates to this file. Keep it as a pointer unless that agent needs separate, repository-specific routing.

## Quick Decision Flow

Before editing, classify the change:

1. **Find the surface in the workspace map.** Use its audience label to decide
   how much public explanation the change needs.
2. **Choose the right source of truth.** Public workflows belong in READMEs,
   examples, and `skills/use-gpui-storybook`; API and internal behavior belong
   in Rustdocs, source-local comments, tests, and snapshots.
3. **Sync workflow changes.** If story registration, `storybook.toml`
   semantics, dock behavior, MCP/capture behavior, macro expansion behavior,
   locale wiring, or public usage guidance changes, update the relevant public
   docs and skill guidance in the same change.
4. **Validate narrowly.** Run the smallest command that proves the edited
   behavior or documentation surface is still sound.

## Audience Labels

These labels describe the crate or surface itself, not the documentation file
being edited:

- **User-facing**: normal entry points for application developers adopting storybook in their app or component workspace.
- **Public integration**: public crates meant for deeper customization, proc-macro usage, or runtime and config integration. These are usually not the default starting point.
- **Internal**: implementation detail crates and workspace plumbing that most consumers should not depend on directly.

## Documentation Placement

Treat the root `README.md`, crate-level `README.md` files, example READMEs under
`examples/`, and `skills/use-gpui-storybook/SKILL.md` as user-facing
documentation.

Even README files for public-integration or internal crates should explain:

- who the crate is for,
- what it does,
- what most users should use instead when applicable.

Keep user-facing documentation example-first. Prefer Rust or TOML snippets over
prose-only explanations when showing behavior changes.

Keep durable internal behavior close to the implementation: Rustdocs for public
API contracts and crate responsibilities, source-local comments for non-obvious
details, and tests, snapshots, or fixtures for executable behavior. Keep this
`AGENTS.md` limited to repository-wide routing and synchronization rules.

Keep maintainer-only details out of `skills/use-gpui-storybook`; that skill is
public application-developer guidance.

## Synchronization Rules

When a substantive change modifies a public workflow, story registration
behavior, `storybook.toml` semantics, dock behavior, MCP/capture behavior,
locale wiring, macro expansion behavior, or other user-visible runtime behavior:

1. Update the root `README.md`.
2. Update `crates/gpui-storybook/README.md` for top-level usage guidance.
3. Update affected public-integration crate READMEs when their direct API or
   contract changed.
4. Update the matching example README when the change affects one registration style.
5. Update `skills/use-gpui-storybook/SKILL.md` when public usage guidance changes.
6. Update Rustdocs for changed public APIs, macro contracts, crate responsibilities, or internal behavior formerly described only in prose docs.
7. Update tests, `insta` snapshots, example code, or fixture crates when they
   are the executable source of truth.

Keep the root `README.md` and `crates/gpui-storybook/README.md` aligned for
top-level usage guidance.

Keep `examples/story/README.md` aligned with explicit `#[story]` workflow
changes.

Keep `examples/component/README.md` aligned with `#[derive(ComponentStory)]`
workflow changes.

Keep `crates/gpui-storybook-toml/README.md`, both example `storybook.toml`
files, root `README.md`, `crates/gpui-storybook/README.md`, and the skill
aligned when `group`, `allow`, `disable_story`, or runtime config resolution
behavior changes.

Keep `crates/gpui-storybook-macros/README.md`, macro Rustdocs, macro tests, and
snapshots aligned when `#[story]`, `#[derive(ComponentStory)]`,
`#[derive(Substory)]`, `#[storybook(...)]`, or `#[story_init]` expansion
behavior changes.

Keep `crates/gpui-storybook-mcp/README.md`, root `README.md`,
`crates/gpui-storybook/README.md`, examples, core automation/capture Rustdocs,
and the skill aligned when MCP tools, capture environment variables, route keys,
or screenshot behavior changes.

Keep `crates/gpui-storybook-core/i18n.toml`, `examples/*/i18n.toml`,
`examples/*/i18n/*/*.ftl`, Rust locale code, examples, and docs aligned when
locale setup or message keys change.

## Workspace Map

### Main User-Facing Entry Points

- `crates/gpui-storybook`
  Audience: **User-facing**
  Docs: [README](crates/gpui-storybook/README.md), crate Rustdocs
  Role: workspace facade, default entry point, and public home for `init`, `generate_stories`, window helpers, story discovery and filtering, and optional macro re-exports.

- `examples/story`
  Audience: **User-facing**
  Docs: [README](examples/story/README.md)
  Role: executable example of the explicit `#[story]` plus `Story` trait workflow.

- `examples/component`
  Audience: **User-facing**
  Docs: [README](examples/component/README.md)
  Role: executable example of component-attached registration with `#[derive(ComponentStory)]`.

- `skills/use-gpui-storybook`
  Audience: **User-facing**
  Docs: [SKILL.md](skills/use-gpui-storybook/SKILL.md)
  Role: public application-developer guidance for setup, story registration, `storybook.toml`, gallery or dock mode, locale wiring, and MCP automation/capture.

### Public Integration Crates

- `crates/gpui-storybook-core`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-core/README.md), crate Rustdocs
  Role: UI runtime for gallery, dock workspace, story containers, title bar, themes, locale wiring, assets, and window shell behavior. Most applications should start with `gpui-storybook` instead.

- `crates/gpui-storybook-macros`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-macros/README.md), macro Rustdocs, `insta` snapshots
  Role: proc macros for `#[story]`, `#[derive(ComponentStory)]`,
  `#[derive(Substory)]`, and `#[story_init]`. Most users should depend on the
  facade crate instead of this crate directly.

- `crates/gpui-storybook-toml`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-toml/README.md), crate Rustdocs, unit tests
  Role: loader and schema boundary for crate-local `storybook.toml` discovery config. Most applications consume this indirectly through `gpui-storybook`.

- `crates/gpui-storybook-mcp`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-mcp/README.md), crate Rustdocs
  Role: MCP tools, stdio serving, environment-driven capture startup, and capture launch helpers exposed through the facade crate's `mcp` feature.

### Internal Crates

- `crates/gpui-storybook-components`
  Audience: **Internal**
  Docs: [README](crates/gpui-storybook-components/README.md), crate Rustdocs
  Role: shared dock-sidebar UI pieces such as `StorySidebarItem` and `StoryDrag` used by the runtime. This is primarily an implementation detail of `gpui-storybook-core`.

- `crates/gpui-storybook-preferences`
  Audience: **Internal**
  Docs: [README](crates/gpui-storybook-preferences/README.md), crate Rustdocs
  Role: typed consumer-scoped Storybook preference intent, atomic JSON documents, Rust-derived JSON Schema, project-local/temporary/disabled storage modes, invalid-file recovery, injected system detectors, and deterministic theme/language resolution. Application code uses the `gpui-storybook` facade.

## Validation and Editing Rules

### Validation After Changes

- `justfile` is the local command index; start with `just --list` when choosing
  repository-wide validation.
- Run the narrowest command that proves the edited behavior works for the
  affected crate, docs, example, or storybook surface.
- Use `just fmt`, `just check`, `just clippy`, `just test`, `just test-docs`,
  `just cov`, or a more specific command when the change spans multiple surfaces.
  `just check` and `just clippy` exclude both example packages; `just test`
  matches CI's workspace test scope. `just cov` measures every publishable
  crate and excludes the two example applications.
- CI also runs an es-fluent FTL check, `cargo fmt --check`,
  `cargo clippy --workspace --all-features`,
  `cargo doc --workspace --all-features --no-deps --locked`,
  `cargo package --workspace --list`, workspace coverage uploaded to Codecov,
  the full workspace test suite on Rust stable across Linux, macOS, and Windows,
  and a cargo-machete action.
- Use `cargo test -p gpui-storybook-preferences --locked` for focused changes
  to typed preference values, JSON/schema repository behavior, invalid-file
  recovery, system detectors, or theme/language resolution.
- If validation cannot be run, state why and what remains unvalidated.
- Do not claim a change works unless it was validated or the remaining risk is
  explicitly documented.

### When Editing Docs

- Keep READMEs user-facing and task-oriented.
- Keep internal implementation details in Rustdocs, source comments, tests, and snapshots.
- Prefer example snippets over prose-only explanations.
- Sync the root `README.md`, affected crate `README.md` files, example
  `README.md` files, Rustdocs, and `skills/use-gpui-storybook` when the workflow changed.

### When Editing Story Registration or Discovery

- Keep `#[story]` and `#[derive(ComponentStory)]` flows consistent in docs unless the change is intentionally specific to one flow.
- Update both example apps when a shared registration concept changes.
- Keep `disable_story` semantics aligned with the registered story type names described in the macro and TOML docs.
- Update root `README.md`, `crates/gpui-storybook/README.md`, macro docs, TOML
  docs, tests, examples, and the skill when group filtering or runtime config
  resolution behavior changes.

### When Editing Runtime UI or Dock Behavior

- Keep gallery and dock terminology consistent across docs.
- Update `crates/gpui-storybook-core` Rustdocs when panel flow, grouping, persistence, or window setup changes.
- Update `crates/gpui-storybook-components/README.md` and Rustdocs when shared dock-sidebar primitives change materially.

### When Editing Tests and Fixtures

- For proc-macro expansion changes, update the inline tests in
  `crates/gpui-storybook-macros/src/lib.rs` and matching snapshots under
  `crates/gpui-storybook-macros/src/snapshots/`.
- For duplicate story-key diagnostics, keep
  `crates/gpui-storybook/tests/duplicate_story_key.rs` aligned with the
  `crates/gpui-storybook/tests/fixtures/duplicate-story-key` fixture crate.
