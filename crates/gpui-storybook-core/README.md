# gpui-storybook-core

`gpui-storybook-core` is the runtime crate behind `gpui-storybook`.

It provides:

- `Gallery` for the searchable sidebar + active story layout.
- `StoryContainer` and the `Story` runtime used by both gallery and dock modes.
- `create_new_window` and `create_new_window_with_ui` for the standard storybook shell.
- The optional dock workspace behind the `dock` feature.
- Theme persistence, locale wiring, asset loading, and title-bar controls.

Most applications should depend on `gpui-storybook` instead of using this crate directly.

Architecture notes: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
