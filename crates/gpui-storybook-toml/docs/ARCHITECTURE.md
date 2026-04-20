# Architecture: gpui-storybook-toml

## Purpose

`gpui-storybook-toml` is the schema and evaluation boundary for `storybook.toml`.

This crate intentionally does not know about inventory, story containers, or working-directory fallback. It only:

- loads `storybook.toml` from a directory
- deserializes and validates the schema
- evaluates allow/deny rules against caller-supplied group and story names

## Schema model

`StorybookToml` is the canonical config shape:

- `group: String`
- `allow: Option<Vec<String>>`
- `disable_story: Vec<String>`

Serde rules:

- unknown keys are rejected through `#[serde(deny_unknown_fields)]`
- `allow` defaults to `None`
- `disable_story` defaults to an empty list
- `group` has no serde default, so the file is invalid when the key is absent

Normalization rules are applied at evaluation time instead of deserialization time:

- `group()` trims whitespace and returns `None` for an empty string
- `allows_group()` trims both the candidate group and each `allow` entry

## Load path

`load_from_dir(dir)` is the only I/O entry point.

It always looks for:

- `<dir>/storybook.toml`

Behavior:

- missing file -> `Ok(None)`
- read failure -> `StorybookTomlError::Read { path, source }`
- parse failure -> `StorybookTomlError::Parse { path, source }`

Errors preserve the full path so callers can log actionable diagnostics.

## Allow and deny evaluation

`allows_group(group)` implements the config semantics used by the facade crate:

- if `allow` is `None`, only the config's own normalized `group` is allowed
- if `allow` is present, any trimmed entry that equals `"*"` allows everything
- otherwise a candidate group must exactly match one of the trimmed `allow` entries
- `None` or empty candidate groups are denied unless wildcard is present

`is_story_disabled(story_name)` is a simple exact-match check against `disable_story`.

The crate deliberately leaves higher-level policy to the caller. For example, the facade crate decides whether the candidate group should come from the story crate's `group` or from the story's declared `section`.

## Test coverage

Unit tests cover:

- missing file handling
- required `group`
- default self-group behavior when `allow` is omitted
- wildcard and explicit allowlists
- empty allowlists
- exact story-name deny matches
