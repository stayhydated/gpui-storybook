default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    rumdl fmt .

# Run with the pinned CI quality tools installed.
quality:
    taplo fmt --check
    rumdl check .
    typos
    cargo sort-derives --check
    actionlint
    git ls-files -z '*.sh' '*.bash' | xargs -0 --no-run-if-empty shellcheck

clippy:
    cargo clippy --workspace --all-features --all-targets --locked -- -D warnings

check:
    cargo check --workspace --all-features --locked \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component

test:
    cargo test --workspace --all-features --locked

cov:
    cargo llvm-cov --workspace --locked \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component \
        --exclude xtask \
        --exclude web \
        --all-features --all-targets

test-publish:
    cargo publish --workspace --dry-run --locked --allow-dirty

test-docs:
    cargo doc --workspace --all-features --no-deps --locked --open

book:
    mdbook serve book

gpui-demo-build:
    cargo xtask build gpui-demo

web-build: gpui-demo-build
    cargo xtask build book
    cargo xtask build llms-txt
    cargo xtask build web

web: web-build
    dx serve --package web

web-preview: web-build
    cargo xtask preview web
