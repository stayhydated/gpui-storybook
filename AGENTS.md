# AGENTS.md

This file is the working guide for contributors and coding agents in the `gpui-storybook` workspace.

Use it to answer three questions quickly:

1. Which crate is the default entry point vs an extension surface vs an internal detail?
1. Where should user-facing documentation live vs internal architecture notes?
1. What other examples, READMEs, or config surfaces must move with the same change?

Ignore all folders matching `**/__crate_paths__/**`.

## Project summary

`gpui-storybook` is a Rust workspace for building and inspecting GPUI components in a storybook-style shell.

Its priorities are:

1. **Fast iteration**: preview component variants without running the full application.
1. **Organization**: group stories into stable sections and top-level runtime groups.
1. **Developer experience**: provide built-in theming, locale switching, story registration helpers, and optional docked layouts.

For most application code, start with `crates/gpui-storybook`.

Reach for the example apps when you need to validate or demonstrate the two supported registration styles:

- `examples/story` for explicit `#[story]` + `Story` implementations.
- `examples/component` for `#[derive(ComponentStory)]` on the component itself.

## Audience labels

These labels describe the crate or surface itself, not the documentation file you are editing:

- **User-facing**: normal entry points for application developers adopting storybook in their app or component workspace.
- **Public integration**: public crates meant for deeper customization, proc-macro usage, or runtime/config integration, but not usually the default starting point.
- **Internal**: implementation detail crates and workspace plumbing that most consumers should not depend on directly.

## Documentation rules

### User-facing documentation

These surfaces are user-facing:

- the root `README.md`,
- crate-level `README.md` files,
- example `README.md` files under `examples/`.

Even for public-integration or internal crates, a `README.md` should explain:

- who the crate is for,
- what it does,
- what most users should use instead when applicable.

### Internal documentation

Only `docs/ARCHITECTURE.md` files are internal documentation.

Use them for:

- story discovery and filtering internals,
- runtime data flow and module boundaries,
- proc-macro parsing and expansion details,
- dock/gallery relationships,
- design rationale and subsystem responsibilities.

Do not put architecture-only implementation detail into READMEs.

## Synchronization rules

When changing a public workflow, story registration behavior, `storybook.toml` semantics, dock behavior, or other user-visible runtime behavior:

1. Update the root `README.md`.
1. Update the affected crate `README.md` files.
1. Update the matching example `README.md` files when the change affects either registration style.
1. Update the relevant `docs/ARCHITECTURE.md` files when the internal flow or crate boundaries changed.
1. Keep these surfaces aligned in the same change unless there is a documented reason not to.

Additional rules:

- User-facing documentation should be example-first.
- Prefer Rust or TOML snippets over prose-only explanations when behavior changes.
- Keep the root `README.md` and `crates/gpui-storybook/README.md` aligned for top-level usage guidance.
- Keep `examples/story/README.md` aligned with explicit `#[story]` workflow changes.
- Keep `examples/component/README.md` aligned with `#[derive(ComponentStory)]` workflow changes.
- Keep `crates/gpui-storybook-toml/README.md`, both example `storybook.toml` files, and the facade docs aligned when `group`, `allow`, or `disable_story` behavior changes.

## Workspace map

### Main user-facing entry points

- `crates/gpui-storybook`
  Audience: **User-facing**
  Docs: [Architecture](crates/gpui-storybook/docs/ARCHITECTURE.md)
  Role: workspace facade, default entry point, and public home for `init`, `generate_stories`, window helpers, story discovery/filtering, and optional macro re-exports.

- `examples/story`
  Audience: **User-facing**
  Docs: [README](examples/story/README.md)
  Role: executable example of the explicit `#[story]` + `Story` trait workflow.

- `examples/component`
  Audience: **User-facing**
  Docs: [README](examples/component/README.md)
  Role: executable example of component-attached registration with `#[derive(ComponentStory)]`.

### Public integration crates

- `crates/gpui-storybook-core`
  Audience: **Public integration**
  Docs: [Architecture](crates/gpui-storybook-core/docs/ARCHITECTURE.md)
  Role: UI runtime for gallery, dock workspace, story containers, title bar, themes, locale wiring, assets, and window shell behavior. Most applications should start with `gpui-storybook` instead.

- `crates/gpui-storybook-macros`
  Audience: **Public integration**
  Docs: [Architecture](crates/gpui-storybook-macros/docs/ARCHITECTURE.md)
  Role: proc macros for `#[story]`, `#[derive(ComponentStory)]`, and `#[story_init]`. Most users should depend on the facade crate instead of this crate directly.

- `crates/gpui-storybook-toml`
  Audience: **Public integration**
  Docs: [Architecture](crates/gpui-storybook-toml/docs/ARCHITECTURE.md)
  Role: loader and schema boundary for crate-local `storybook.toml` discovery config. Most applications consume this indirectly through `gpui-storybook`.

### Internal crates

- `crates/gpui-storybook-components`
  Audience: **Internal**
  Docs: [README](crates/gpui-storybook-components/README.md)
  Role: shared dock-sidebar UI pieces such as `StorySidebarItem` and `StoryDrag` used by the runtime. This is primarily an implementation detail of `gpui-storybook-core`.

## Working rules by change type

### When editing docs

- Keep READMEs user-facing and task-oriented.
- Move discovery internals, runtime boundaries, and macro expansion details into `docs/ARCHITECTURE.md`.
- Prefer example snippets over prose-only explanations.
- Sync the root README, affected crate README files, and example README files in the same change when the workflow changed.

### When editing Rust crates

- Use `cargo` for build, test, and run tasks.
- Use `cargo fmt` for Rust formatting and `taplo fmt` for TOML formatting.
- Keep shared dependency versions in the workspace root `Cargo.toml`.
- Prefer `workspace = true` for workspace dependencies where applicable.

### When editing story registration or discovery

- Keep `#[story]` and `#[derive(ComponentStory)]` flows consistent in docs unless the change is intentionally specific to one flow.
- Update both example apps when a shared registration concept changes.
- Keep `disable_story` semantics aligned with the registered story type names described in the macro and TOML docs.
- Update both the facade and TOML docs when group filtering or discovery fallback behavior changes.

### When editing runtime UI or dock behavior

- Keep gallery and dock terminology consistent across docs.
- Update `crates/gpui-storybook-core/docs/ARCHITECTURE.md` when panel flow, grouping, persistence, or window setup changes.
- Update `crates/gpui-storybook-components/README.md` when shared dock-sidebar primitives change materially.

### When writing tests

- Use `cargo test --workspace --all-features` for the full suite.
- Prefer `insta` for proc-macro expansion snapshots when it fits better than assertion-heavy tests.
- Prefer readable multiline Rust snippets in macro tests over escaped single-line literals.

### When validating changes locally

- Check the workspace with:
  `cargo check --workspace --all-features --exclude gpui-storybook-example-story --exclude gpui-storybook-example-component`
- Run the example apps when the change affects registration, discovery, theming, locale behavior, or dock UI:
  `cargo run -p gpui-storybook-example-story`
  `cargo run -p gpui-storybook-example-component`
