# AGENTS.md

This is the working guide for contributors and coding agents in the
`gpui-storybook` workspace.

Use it to decide:

1. where to start,
2. whether a crate or surface is user-facing, public integration, or internal,
3. which docs, examples, skill guidance, tests, and generated snapshots must
   change together,
4. which validation command should run before handoff.

For most application-facing code and docs, start with `crates/gpui-storybook`.

Reach for the example apps when you need to understand or demonstrate the two
supported registration styles:

- `examples/story` for explicit `#[story]` plus `Story` implementations.
- `examples/component` for `#[derive(ComponentStory)]` on the component itself.

`CLAUDE.md` delegates to this file. Keep it as a pointer unless that agent needs
separate, repository-specific routing.

## Project Summary

`gpui-storybook` is a Rust workspace for building and inspecting GPUI
components in a storybook-style shell.

Its priorities are:

1. **Fast iteration**: preview component variants without running the full application.
2. **Organization**: group stories into stable sections and top-level runtime groups.
3. **Developer experience**: provide built-in theming, locale switching, story registration helpers, and optional docked layouts.

## Quick Decision Flow

Before editing, classify the change:

1. **Find the surface in the workspace map.** Use its audience label to decide
   how much public explanation the change needs.
2. **Choose the right source of truth.** Public workflows belong in READMEs,
   examples, and `skills/use-gpui-storybook`; API and internal behavior belong
   in Rustdocs, source-local comments, tests, and snapshots.
3. **Sync workflow changes.** If story registration, `storybook.toml`
   semantics, dock behavior, generated output, locale wiring, or recommended
   usage changes, update the relevant public docs and skill guidance in the same
   change.
4. **Validate narrowly.** Run the smallest command that proves the edited
   behavior or documentation surface is still sound.

## Audience Labels

These labels describe the crate or surface itself, not the documentation file
being edited:

- **User-facing**: normal entry points for application developers adopting storybook in their app or component workspace.
- **Public integration**: public crates meant for deeper customization, proc-macro usage, or runtime and config integration. These are usually not the default starting point.
- **Internal**: implementation detail crates and workspace plumbing that most consumers should not depend on directly.

## Documentation Placement

### User-Facing Documentation

Treat these surfaces as user-facing:

- the root `README.md`,
- crate-level `README.md` files,
- example `README.md` files under `examples/`,
- `skills/use-gpui-storybook/SKILL.md`.

Even README files for public-integration or internal crates should explain:

- who the crate is for,
- what it does,
- what most users should use instead when applicable.

Keep user-facing documentation example-first. Prefer Rust or TOML snippets over
prose-only explanations when showing behavior changes.

There is no mdBook/book surface in the current workspace. Do not create one just
to document an ordinary workflow change; if a book is added later, include it in
the synchronization checks for affected user-facing workflows.

### Internal Documentation

Do not add new crate-level `docs/ARCHITECTURE.md` files. The previous
architecture notes were removed; durable internal behavior should now live close
to the implementation:

- crate-level or item-level Rustdocs for public API contracts and crate responsibilities,
- source-local comments for non-obvious implementation details,
- unit tests, snapshot tests, and fixtures for executable behavior,
- this `AGENTS.md` only for repository-wide routing and synchronization rules.

Keep maintainer-only details out of `skills/use-gpui-storybook`; that skill is
public application-developer guidance.

## Synchronization Rules

When a substantive change modifies a public workflow, story registration
behavior, `storybook.toml` semantics, dock behavior, locale wiring, generated
output, or other user-visible runtime behavior:

1. Update the root `README.md`.
2. Update `crates/gpui-storybook/README.md` for top-level usage guidance.
3. Update affected public-integration crate READMEs when their direct API or
   contract changed.
4. Update the matching example README when the change affects one registration style.
5. Update `skills/use-gpui-storybook/SKILL.md` when public usage guidance changes.
6. Update Rustdocs for changed public APIs, macro contracts, crate responsibilities, or internal behavior formerly described only in prose docs.
7. Update tests, `insta` snapshots, example code, or `storybook.toml` fixtures when they are the executable source of truth.

Keep the root `README.md` and `crates/gpui-storybook/README.md` aligned for
top-level usage guidance.

Keep `examples/story/README.md` aligned with explicit `#[story]` workflow
changes.

Keep `examples/component/README.md` aligned with `#[derive(ComponentStory)]`
workflow changes.

Keep `crates/gpui-storybook-toml/README.md`, both example `storybook.toml`
files, facade docs, and the skill aligned when `group`, `allow`,
`disable_story`, or runtime config resolution behavior changes.

Keep `crates/gpui-storybook-macros/README.md`, macro Rustdocs, macro tests, and
snapshots aligned when `#[story]`, `#[derive(ComponentStory)]`,
`#[storybook(...)]`, or `#[story_init]` expansion behavior changes.

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

### Public Integration Crates

- `crates/gpui-storybook-core`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-core/README.md), crate Rustdocs
  Role: UI runtime for gallery, dock workspace, story containers, title bar, themes, locale wiring, assets, and window shell behavior. Most applications should start with `gpui-storybook` instead.

- `crates/gpui-storybook-macros`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-macros/README.md), macro Rustdocs, `insta` snapshots
  Role: proc macros for `#[story]`, `#[derive(ComponentStory)]`, and `#[story_init]`. Most users should depend on the facade crate instead of this crate directly.

- `crates/gpui-storybook-toml`
  Audience: **Public integration**
  Docs: [README](crates/gpui-storybook-toml/README.md), crate Rustdocs, unit tests
  Role: loader and schema boundary for crate-local `storybook.toml` discovery config. Most applications consume this indirectly through `gpui-storybook`.

### Internal Crates

- `crates/gpui-storybook-components`
  Audience: **Internal**
  Docs: [README](crates/gpui-storybook-components/README.md), crate Rustdocs
  Role: shared dock-sidebar UI pieces such as `StorySidebarItem` and `StoryDrag` used by the runtime. This is primarily an implementation detail of `gpui-storybook-core`.

## Validation and Editing Rules

### Validation After Changes

- Validation is the default after code or workflow changes.
- Run the narrowest command that proves the edited behavior works for the
  affected crate, docs, example, or storybook surface.
- Prefer targeted crate, example, docs, or UI checks before full-workspace validation.
- Use `just check`, `just test`, `just test-docs`, or a more specific command when the change spans multiple surfaces.
- If validation cannot be run, state why and what remains unvalidated.
- Do not claim a change works unless it was validated, generated from a source of truth, or the remaining risk is explicitly documented.

### When Editing Docs

- Keep READMEs user-facing and task-oriented.
- Keep internal implementation details in Rustdocs, source comments, tests, and snapshots.
- Prefer example snippets over prose-only explanations.
- Sync the root `README.md`, affected crate `README.md` files, example
  `README.md` files, Rustdocs, and `skills/use-gpui-storybook` when the workflow changed.

### When Editing Rust Crates

- Use `cargo` for build, test, and run tasks.
- Use `cargo fmt` for Rust formatting and `taplo fmt` for TOML formatting.
- Keep shared dependency versions in the workspace root `Cargo.toml`.
- Prefer `workspace = true` for workspace dependencies where applicable.

### When Editing Story Registration or Discovery

- Keep `#[story]` and `#[derive(ComponentStory)]` flows consistent in docs unless the change is intentionally specific to one flow.
- Update both example apps when a shared registration concept changes.
- Keep `disable_story` semantics aligned with the registered story type names described in the macro and TOML docs.
- Update facade docs, macro docs, TOML docs, tests, examples, and the skill when group filtering or runtime config resolution behavior changes.

### When Editing Runtime UI or Dock Behavior

- Keep gallery and dock terminology consistent across docs.
- Update `crates/gpui-storybook-core` Rustdocs when panel flow, grouping, persistence, or window setup changes.
- Update `crates/gpui-storybook-components/README.md` and Rustdocs when shared dock-sidebar primitives change materially.

### When Writing Tests

- Prefer `insta` for proc-macro expansion snapshots when it fits better than assertion-heavy tests.
- Prefer readable multiline Rust snippets in macro tests over escaped single-line literals.
