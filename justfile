default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    rumdl fmt .

clippy:
    cargo clippy --workspace --all-features --all-targets --locked \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component -- -D warnings

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
