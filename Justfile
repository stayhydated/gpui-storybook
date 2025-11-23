default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt

p-lib-forms:
  cargo run -p prototyping

test-publish:
  ES_FLUENT_SKIP_BUILD=true cargo publish --workspace --dry-run --allow-dirty
