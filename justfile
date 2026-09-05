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

test:
    @cargo test --workspace --lib --bins --tests

bench:
    @cargo bench --workspace

# cargo +stable install cargo-llvm-cov --locked
# rustup component add llvm-tools-preview
test-cov:
    @cargo llvm-cov --workspace --lib --bins --tests

fmt:
    @cargo +nightly fmt --all

fmt-check:
    @cargo +nightly fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty --all
    @cargo +nightly fmt

lint: fmt-check clippy

