#!/usr/bin/env bash
set -euo pipefail

selection=${1:?Pass default, no-default, macros, inspector, performance, or mcp}
dependency='default-features = false'
api='pub use gpui_storybook::StorybookWindow;'
case "$selection" in
  default)
    dependency='default-features = true'
    api+=' pub use gpui_storybook::story;'
    ;;
  no-default) ;;
  macros)
    dependency+=', features = ["macros"]'
    api+=' pub use gpui_storybook::story;'
    ;;
  inspector)
    dependency+=', features = ["inspector"]'
    api+=' pub use gpui_storybook::StoryInspectorState;'
    ;;
  performance) dependency+=', features = ["performance"]' ;;
  mcp)
    dependency+=', features = ["mcp"]'
    api+=' pub use gpui_storybook::mcp::StorybookMcpServerOptions;'
    ;;
  *) printf 'Unknown feature selection: %s\n' "$selection" >&2; exit 2 ;;
esac

repo_dir=$(pwd)
consumer=$(mktemp -d)
trap 'rm -rf "$consumer"' EXIT
mkdir "$consumer/src"
cat > "$consumer/Cargo.toml" <<EOF
[package]
name = "storybook-feature-consumer"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
gpui-storybook = { path = "$repo_dir/crates/gpui-storybook", $dependency }
EOF
printf '%s\n' "$api" > "$consumer/src/lib.rs"

# Seed resolution with the workspace lock, then admit the new consumer root.
# No workspace members or dev-dependencies can enable facade features here.
cp Cargo.lock "$consumer/Cargo.lock"
cargo metadata --manifest-path "$consumer/Cargo.toml" --format-version 1 > /dev/null
cargo check --manifest-path "$consumer/Cargo.toml" --lib --locked \
  --target-dir "$repo_dir/target/feature-consumers"
