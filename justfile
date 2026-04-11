default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    uvx mdformat .

clippy:
    cargo clippy --workspace --all-features \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component

check:
    cargo check --workspace --all-features \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component

test:
    cargo test --workspace --all-features

test-publish:
    cargo publish --workspace --dry-run --allow-dirty

test-docs:
    cargo doc --workspace --all-features --no-deps --open
