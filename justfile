default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    uvx mdformat .

lint:
    cargo clippy --workspace --all-features
    cargo machete

p-lib-forms:
    cargo run -p prototyping

test-publish:
    cargo publish --workspace --dry-run --allow-dirty
