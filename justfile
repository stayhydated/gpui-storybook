default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    rumdl fmt .

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

cov:
    cargo llvm-cov --workspace \
        --exclude gpui-storybook-example-story \
        --exclude gpui-storybook-example-component \
        --exclude xtask \
        --exclude web \
        --all-features --all-targets

test-publish:
    cargo publish --workspace --dry-run --allow-dirty

test-docs:
    cargo doc --workspace --all-features --no-deps --open

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
