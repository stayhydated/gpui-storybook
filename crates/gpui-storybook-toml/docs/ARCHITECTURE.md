# Architecture

## Purpose

`gpui-storybook-toml` loads `storybook.toml` files used by story discovery. Callers typically pass either a crate manifest directory or the current working directory.

## Config schema

- `group` (required string when file exists): Assigns the crate's top-level runtime discovery group for `allow` filtering and sidebar grouping. It does not overwrite a story's declared section within that group.
- `allow` (optional string array): Allowed top-level group identifiers.
  - Omitted `allow` allows only the config's own `group`.
  - `allow = ["*"]` includes every group.
  - `allow = []` includes none.
- `disable_story` (optional string array): Per-story denylist by registered story name.

## API

- `load_from_dir`: Reads `<crate-dir>/storybook.toml` and returns `Option<StorybookToml>`.
- `StorybookToml::allows_group`: Evaluates the group allowlist.
- `StorybookToml::is_story_disabled`: Evaluates per-story denylist membership.
- `StorybookToml::group`: Returns a trimmed non-empty group name.
- Errors include the file path so the caller can log actionable parse and read failures.
