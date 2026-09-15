# Working in gpui-storybook

Start with `crates/gpui-storybook` for application-facing changes and
`just --list` for workspace commands. The executable examples show the two
registration styles: `examples/story` uses `#[story]` with `Story`, while
`examples/component` uses `#[derive(ComponentStory)]`.

## Where changes belong

| Surface | Audience and responsibility |
|---|---|
| `crates/gpui-storybook` | Application facade: initialization, discovery, filtering, and public re-exports |
| `crates/gpui-storybook-core` | Runtime integration: gallery, dock, workbench, story containers, preferences UI, and automation |
| `crates/gpui-storybook-macros` | Public macro syntax and generated registrations, controls, and substory keys |
| `crates/gpui-storybook-toml` | Public configuration schema, loading, and filters |
| `crates/gpui-storybook-mcp` | Linux/macOS MCP tools, stdio serving, and capture launch helpers |
| `crates/gpui-storybook-launch` | Standalone Linux command that owns the headless Sway lifecycle |
| `crates/gpui-storybook-test` | Public test integration: fresh story contexts, captures, matrices, baselines, and frame budgets |
| `crates/gpui-storybook-components` | Internal sidebar and drag components used by the runtime |
| `crates/gpui-storybook-preferences` | Internal typed persistence, system detection, and preference resolution |
| `examples/story`, `examples/component` | Executable application examples and registration fixtures |
| `book/src` | Application user guide; navigate through `SUMMARY.md` |
| `skills/use-gpui-storybook` | Application integration guidance for coding agents |
| `web/src/lib.rs` | Public catalog and site routes |
| `xtask` | Book, LLM text, demo, and site build orchestration |

Keep API contracts in Rustdocs and implementation details beside the source,
tests, snapshots, or fixtures that establish them. The book and integration
skill serve application developers.

## Keep related surfaces aligned

Update the public descriptions that cover changed behavior: root and facade
READMEs, the affected crate README, matching book sections, examples, and the
integration skill's relevant reference. Update `web/src/lib.rs` when its catalog
copy describes that behavior.

Keep READMEs focused on purpose, usage, and relevant constraints. Use CI,
Codecov, book, and crates.io badges for their destinations; leave installation
instructions and book or API navigation to those linked surfaces.

- **Registration and controls:** keep macro Rustdocs,
  `crates/gpui-storybook-macros/src/tests.rs`, and its `src/snapshots/` aligned.
  Update both example styles when a shared registration concept changes.
- **Duplicate keys:** keep
  `crates/gpui-storybook/tests/duplicate_story_key.rs` aligned with
  `crates/gpui-storybook/tests/fixtures/duplicate-story-key`.
- **Configuration:** synchronize TOML field semantics and runtime selection with
  both example `storybook.toml` files. `disable_story` matches registered type
  names; display titles and route keys are separate identities.
- **Runtime and dock behavior:** update the owning core Rustdocs and runtime
  tests. Update the components README when shared sidebar primitives change.
- **Automation and capture:** keep MCP schemas, core automation/capture
  contracts, examples, and the automation skill reference aligned.
- **Portable tests:** keep the test crate README, portable-testing and automation
  book sections, examples, and automation skill reference aligned when capture
  matrices, baseline policy, context setup, or frame budgets change.
- **Localization:** keep Rust locale code, core and example `i18n.toml` files,
  affected FTL catalogs, and locale setup instructions aligned when message keys
  or locale wiring change.

Build publication artifacts through `cargo xtask`: the sources are `book/src`,
`web/src`, and `examples/story`. The generated book, LLM text, demo, and site
outputs are not independent editing surfaces.

## Validate the changed surface

Choose the narrowest relevant check and report its result, including any
unexecuted or failed checks.

| Change | Validation |
|---|---|
| Markdown | `rumdl check` with the edited paths |
| Book or LLM text | `cargo xtask build book` and `cargo xtask build llms-txt` |
| Macro expansion | `cargo test -p gpui-storybook-macros --locked` |
| Preference storage or resolution | `cargo test -p gpui-storybook-preferences --locked` |
| Portable runner | `cargo test -p gpui-storybook-test --all-features --locked` |
| Public Rust API docs | `cargo doc --workspace --all-features --no-deps --locked` on Linux |
| GPUI demo | `cargo xtask build gpui-demo` |
| Catalog | `cargo xtask build web` |

`just fmt` formats Rust, TOML, and Markdown. `just check` and `just clippy`
exclude the two example packages; `just test` includes them. `just web-build`
assembles all publication artifacts.

CI tests all features on Linux. On macOS it excludes the Linux-only launcher;
on Windows it uses default features and excludes the launcher and MCP crate.
Use the matching platform scope when reproducing those jobs.
