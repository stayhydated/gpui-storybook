# Architecture

## Purpose

`gpui-storybook-toml` loads crate-local `storybook.toml` files used by story discovery.

## Config schema

- `group` (required string when file exists): Assigns the crate's runtime discovery group for `allow` filtering and top-level sidebar grouping. It does not overwrite a story's declared section within that group.
- `allow` (optional string array): Allowed group identifiers.
  - Omitted `allow` allows only the config's own `group`.
  - `allow = ["*"]` includes all stories.
  - `allow = []` includes none.
- `disable_story` (optional string array): Per-story denylist by story struct name.

## API

- `load_from_dir`: Reads `<crate-dir>/storybook.toml` and returns `Option<StorybookToml>`.
- `StorybookToml::allows_group`: Evaluates the group allowlist.
- `StorybookToml::is_story_disabled`: Evaluates per-story denylist membership.
- `StorybookToml::group`: Returns a trimmed non-empty group name.
