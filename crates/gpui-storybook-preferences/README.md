# gpui-storybook-preferences

`gpui-storybook-preferences` is the internal typed persistence and resolution
engine for GPUI Storybook preferences. It owns consumer-scoped documents, explicit
persistence modes, system detection, saved intent, effective presentation, and
diagnostics. Applications configure preferences through `StorybookOptions`
from the [`gpui-storybook` facade][facade].

Requires Rust 1.99 or newer.

See the [preference API source][api] for persistence modes, cancellation,
recovery, and saved-versus-resolved preference semantics.

[facade]: https://crates.io/crates/gpui-storybook
[api]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-preferences/src/lib.rs
