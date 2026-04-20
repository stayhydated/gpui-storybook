# Architecture: gpui-storybook

## Purpose

`gpui-storybook` is the public facade crate. It owns story discovery and `storybook.toml` filtering, re-exports the runtime shell from `gpui-storybook-core`, and optionally re-exports proc macros from `gpui-storybook-macros`.

This crate is the boundary where compile-time registration metadata becomes runtime `StoryContainer` values.

## Responsibilities

- re-export user-facing runtime types such as `Gallery`, `Story`, `StoryContainer`, window helpers, and language/locale types
- optionally re-export proc macros behind the `macros` feature
- publish hidden `__registry` and `__inventory` handles so macro expansions can submit inventory entries through the facade path
- install global language and locale state in `init`
- discover, filter, sort, and instantiate stories in `generate_stories`

## Discovery pipeline

1. `#[story]` and `#[derive(ComponentStory)]` expansions submit `gpui_storybook_core::registry::StoryEntry` records into `inventory`.
2. `#[story_init]` expansions submit `InitEntry` records into the same inventory system.
3. `init` stores `CurrentLanguage<L>`, installs `LocaleManager<L>` as the `LocaleStore`, delegates runtime startup to `gpui_storybook_core::story::init`, optionally registers dock panels, then executes discovered init hooks.
4. `generate_stories` iterates all `StoryEntry` records, resolves crate-local config, applies runtime filtering, sorts the surviving entries, and materializes `StoryContainer` entities by calling each entry's `create_fn`.

## Runtime config resolution

`generate_stories` uses two config scopes:

- crate-local config: loaded from `entry.crate_dir/storybook.toml` for every discovered story crate and cached in a `HashMap<&'static str, Option<StorybookToml>>`
- runtime config: the single `storybook.toml` used as the active filter for the current process

Runtime config selection is intentionally heuristic:

1. derive the current binary name from `argv[0]`
2. if a discovered `StoryEntry` has `crate_name == current_binary_name`, reuse that crate's cached config
3. otherwise walk upward from the current working directory until a `storybook.toml` is found

That behavior lets example apps and workspace-local binaries resolve their own config without requiring explicit configuration wiring.

## Filtering semantics

`resolve_story_entry` computes a filter key as:

- the crate-local `group` from that story's `storybook.toml`, if present
- otherwise the story's declared `section`

That filter key is then checked against the runtime config:

- `allow` is evaluated through `StorybookToml::allows_group`
- `disable_story` is evaluated against `entry.name`

`entry.name` is the registered story type name. For `ComponentStory`, that name is the component type name, not the generated wrapper type name.

When a story survives filtering, the facade preserves both:

- `group`: top-level crate grouping from the story crate's `storybook.toml`
- `section`: the story's own section label

The UI runtime uses `group` as the outer bucket and `section` as the nested label inside it.

## Ordering

The final sort order is:

1. `section_order` when both entries have one
2. ordered sections before unordered sections
3. section label
4. story name

`section_order` is produced by macro expansion when a story uses an enum variant section; the discriminant is cast to `usize`.

## Dependency edges

- depends on `gpui-storybook-core` for runtime types, registry definitions, and window helpers
- depends on `gpui-storybook-toml` for config loading and allow/deny evaluation
- optionally depends on `gpui-storybook-macros` behind the `macros` feature
- depends on `inventory` because this crate is the stable facade path used by macro-generated `inventory::submit!` code

## Feature boundaries

- `macros`: re-exports proc macros from `gpui-storybook-macros`
- `dock`: re-exports dock workspace types and helpers from `gpui-storybook-core`
