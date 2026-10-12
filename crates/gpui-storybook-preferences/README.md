# gpui-storybook-preferences

`gpui-storybook-preferences` is the internal typed persistence and resolution
engine for GPUI Storybook preferences. It owns consumer-scoped documents, explicit
persistence modes, system detection, saved intent, effective presentation, and
diagnostics. Applications configure preferences through `StorybookOptions`
from the [`gpui-storybook` facade][facade].

Requires Rust 1.99 or newer.

See the [repository API reference][api] for persistence modes, cancellation,
recovery, and saved-versus-resolved preference semantics. Direct consumers are
Storybook's runtime and persistence tests.

[facade]: https://crates.io/crates/gpui-storybook
[api]: https://docs.rs/gpui-storybook-preferences
