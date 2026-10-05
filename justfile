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

# Build the opted-in example with the explicitly configured Android SDK/NDK.
mobile-build abi="x86_64":
    python3 examples/mobile/build.py --automation --profile mobile \
        --sdk "$ANDROID_HOME" --ndk "$ANDROID_NDK_HOME" --abi "{{abi}}"

# Install and launch the example on one selected device, then serve MCP.
mobile-host serial abi="x86_64":
    cargo run -p gpui-storybook-mobile-host --locked -- \
        --serial "{{serial}}" --allow-interaction \
        --install "target/mobile-example/{{abi}}/storybook.apk" --launch-example

# Verify an already running, opted-in example on an owned emulator.
mobile-test serial:
    python3 examples/mobile/verify.py --serial "{{serial}}"
    STORYBOOK_ANDROID_SERIAL="{{serial}}" cargo test -p gpui-storybook-mobile-host --test android --locked -- --ignored
    python3 examples/mobile/verify_native.py --serial "{{serial}}"
    python3 examples/mobile/native/build.py --sdk "$ANDROID_HOME"
    python3 examples/mobile/native/verify.py --serial "{{serial}}"

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
