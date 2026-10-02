# gpui-storybook-preferences

`gpui-storybook-preferences` is the internal typed persistence and resolution
engine for GPUI Storybook preferences. It owns consumer-scoped documents, explicit
persistence modes, system detection, saved intent, effective presentation, and
diagnostics.

File-backed mutations retain exclusive access across repository clones through
their disk operation and successful cache update. Cancelling an admitted mutation
stops waiting for its result; the mutation still finishes. Cancelling while waiting
for access leaves preferences unchanged.

Application developers configure this behavior through `StorybookOptions` from
the [`gpui-storybook` facade][facade].

[facade]: https://crates.io/crates/gpui-storybook
