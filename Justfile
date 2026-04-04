default: check

init:
    rustup component add clippy rustfmt

build:
    cargo build

install:
    cargo install --path . --force

run *ARGS:
    cargo run -- {{ARGS}}

test:
    cargo test

lint:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

check-fmt:
    cargo fmt -- --check

publish:
    cargo publish --dry-run

check: check-fmt lint test

ci: check-fmt lint build test
