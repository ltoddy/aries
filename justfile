install:
    @cargo +stable install --path crates/aries-cli --locked

run:
    @cargo run

build:
    @cargo build

check:
    @cargo check

clippy:
    @cargo clippy

fmt:
    @cargo +nightly fmt --all

fmt-check:
    @cargo +nightly fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty
    @cargo +nightly fmt

lint: fmt-check clippy

