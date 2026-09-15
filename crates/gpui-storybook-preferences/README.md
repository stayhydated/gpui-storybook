# gpui-storybook-preferences

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook-preferences.svg)](https://crates.io/crates/gpui-storybook-preferences)

`gpui-storybook-preferences` is the typed storage and resolution engine behind
GPUI Storybook preferences. It owns consumer-scoped documents, persistence
modes, the saved Gallery/Dock window-mode enum, system detection, fallback
resolution, and diagnostics.

Application developers configure preferences through `StorybookOptions` from
the [`gpui-storybook`](../gpui-storybook/README.md) facade. Use
`PreferenceState::saved` for user intent and `PreferenceState::resolved` for
effective presentation and its sources.
