# Architecture

## Purpose

`gpui-storybook-toml` loads crate-local `storybook.toml` files used by story discovery.

## Config schema

- `group` (required string when file exists): Overrides the section/group assigned to every discovered story in that crate.
- `allow` (optional string array): Story struct names or group names allowed for that crate.
  - Omitted `allow` includes all stories in the crate.
  - `allow = ["*"]` includes all stories.
  - `allow = []` includes none.

## API

- `load_from_dir`: Reads `<crate-dir>/storybook.toml` and returns `Option<StorybookToml>`.
- `StorybookToml::allows`: Evaluates the allowlist for an individual story name and group.
- `StorybookToml::group`: Returns a trimmed non-empty group name.
