# AGENTS.md

This is the working guide for contributors and coding agents in the
`gpui-storybook` workspace.

Use it to decide:

1. where documentation belongs,
2. whether a crate or surface is user-facing, public integration, or internal,
3. which related docs, examples, and skills must change together,
4. which validation command should run before handoff.

For most application code, start with `crates/gpui-storybook`.

Reach for the example apps when you need to understand or demonstrate the two
supported registration styles:

- `examples/story` for explicit `#[story]` plus `Story` implementations.
- `examples/component` for `#[derive(ComponentStory)]` on the component itself.

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
2. **Place documentation by content, not by crate audience.** README files are
   always user-facing. Internal design belongs in the matching
   `docs/ARCHITECTURE.md`.
3. **Sync public workflow changes.** If story registration, `storybook.toml`
   semantics, dock behavior, generated output, locale wiring, or recommended
   usage changes, update the relevant README, example, architecture note, and
   `.agents/skills/*` guidance in the same change when applicable.
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
- example `README.md` files under `examples/`.

Even README files for public-integration or internal crates should explain:

- who the crate is for,
- what it does,
- what most users should use instead when applicable.

Keep user-facing documentation example-first. Prefer Rust or TOML snippets over
prose-only explanations when showing behavior changes.

### Internal Documentation

Use the relevant `docs/ARCHITECTURE.md` file for internal documentation, such
as the crate-level paths listed in the workspace map.

Keep these topics in architecture documents, not in READMEs:

- implementation details,
- story discovery and filtering internals,
- runtime data flow and module boundaries,
- proc-macro parsing and expansion details,
- dock and gallery relationships,
- design rationale and subsystem responsibilities.

### Skill Guidance

`.agents/skills/use-gpui-storybook` is hosted in this repository as public
GPUI Storybook usage guidance for application developers. It is not internal
architecture, maintenance, CI, release, or contributor-only workflow
documentation.

Update relevant in-repository `.agents/skills/*` guidance when a code change
alters user-facing workflows, story registration behavior, `storybook.toml`
semantics, dock behavior, locale setup, generated output, or recommended usage.

## Synchronization Rules

When a substantive change modifies a public workflow, story registration
behavior, `storybook.toml` semantics, dock behavior, locale wiring, or other
user-visible runtime behavior:

1. Update the root `README.md`.
2. Update the affected crate `README.md` files.
3. Update the matching example `README.md` files when the change affects either registration style.
4. Update relevant in-repository `.agents/skills/*` guidance.
5. Update the relevant `docs/ARCHITECTURE.md` files when the internal flow or crate boundaries changed.
6. Keep these surfaces aligned in the same change unless there is a documented reason not to.

Keep the root `README.md` and `crates/gpui-storybook/README.md` aligned for
top-level usage guidance.

Keep `examples/story/README.md` aligned with explicit `#[story]` workflow
changes.

Keep `examples/component/README.md` aligned with `#[derive(ComponentStory)]`
workflow changes.

Keep `crates/gpui-storybook-toml/README.md`, both example `storybook.toml`
files, and the facade docs aligned when `group`, `allow`, `disable_story`, or
runtime config resolution behavior changes.

## Workspace Map

### Main User-Facing Entry Points

- `crates/gpui-storybook`
  Audience: **User-facing**
  Docs: [Architecture](crates/gpui-storybook/docs/ARCHITECTURE.md)
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

### Internal Crates

- `crates/gpui-storybook-components`
  Audience: **Internal**
  Docs: [README](crates/gpui-storybook-components/README.md)
  Role: shared dock-sidebar UI pieces such as `StorySidebarItem` and `StoryDrag` used by the runtime. This is primarily an implementation detail of `gpui-storybook-core`.

## Validation and Editing Rules

### Validation After Changes

- Validation is the default after code or workflow changes.
- Run the narrowest command that proves the edited behavior works for the
  affected crate, docs, example, or storybook surface.
- Prefer targeted crate, example, docs, or UI checks before full-workspace validation.
- Use `just check`, `just test`, or a more specific `justfile` recipe when the change spans multiple surfaces.
- If validation cannot be run, state why and what remains unvalidated.
- Do not claim a change works unless it was validated, generated from a source of truth, or the remaining risk is explicitly documented.

### When Editing Docs

- Keep READMEs user-facing and task-oriented.
- Move discovery internals, runtime boundaries, and macro expansion details into `docs/ARCHITECTURE.md`.
- Prefer example snippets over prose-only explanations.
- Sync the root `README.md`, affected crate `README.md` files, example `README.md` files, and `.agents/skills/*` guidance in the same change when the workflow changed.

### When Editing Rust Crates

- Use `cargo` for build, test, and run tasks.
- Use `cargo fmt` for Rust formatting and `taplo fmt` for TOML formatting.
- Keep shared dependency versions in the workspace root `Cargo.toml`.
- Prefer `workspace = true` for workspace dependencies where applicable.

### When Editing Story Registration or Discovery

- Keep `#[story]` and `#[derive(ComponentStory)]` flows consistent in docs unless the change is intentionally specific to one flow.
- Update both example apps when a shared registration concept changes.
- Keep `disable_story` semantics aligned with the registered story type names described in the macro and TOML docs.
- Update both the facade and TOML docs when group filtering or runtime config resolution behavior changes.

### When Editing Runtime UI or Dock Behavior

- Keep gallery and dock terminology consistent across docs.
- Update `crates/gpui-storybook-core/docs/ARCHITECTURE.md` when panel flow, grouping, persistence, or window setup changes.
- Update `crates/gpui-storybook-components/README.md` when shared dock-sidebar primitives change materially.

### When Writing Tests

- Prefer `insta` for proc-macro expansion snapshots when it fits better than assertion-heavy tests.
- Prefer readable multiline Rust snippets in macro tests over escaped single-line literals.
