# gpui-storybook-toml

Loader for crate-local `storybook.toml` files used by `gpui-storybook` story discovery.

Schema:

- `group`: required when the file exists.
- `allow`: optional allowlist of runtime groups.
- `disable_story`: optional denylist of registered story type names.

API:

- `load_from_dir` reads `<dir>/storybook.toml`.
- `StorybookToml::allows_group` evaluates runtime group visibility.
- `StorybookToml::is_story_disabled` filters individual stories.

Architecture notes: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
