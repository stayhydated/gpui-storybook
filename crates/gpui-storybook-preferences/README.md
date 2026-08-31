# gpui-storybook-preferences

`gpui-storybook-preferences` is the typed storage and resolution engine behind
GPUI Storybook preferences. It owns consumer-scoped documents, persistence
modes, the saved Gallery/Dock window-mode enum, system detection, fallback
resolution, and diagnostics.

Application developers should configure preferences through
[`StorybookOptions`](https://docs.rs/gpui-storybook/latest/gpui_storybook/struct.StorybookOptions.html)
from the `gpui-storybook` facade. See [Preferences](../../book/src/preferences.md)
for the user-facing behavior.
