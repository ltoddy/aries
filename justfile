install-cli:
    @cargo install --path apps/aries-cli --force --locked

install-orbit:
    @cargo install --path apps/aries-orbit --force --locked

run:
    @cargo run

build:
    @cargo build

check:
    @cargo check --workspace --all-targets

clippy:
    @cargo clippy --workspace --all-targets

test:
    @cargo test --workspace --lib --bins --tests

bench:
    @cargo bench --workspace

# cargo install cargo-llvm-cov --locked
# rustup component add llvm-tools-preview
test-cov:
    @cargo llvm-cov --workspace --lib --bins --tests

fmt:
    @cargo fmt --all

fmt-check:
    @cargo fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty --all
    @cargo fmt

lint: fmt-check clippy

orbit-dmg:
    @cargo build --release --locked --bin aries-orbit
    @bash misc/orbit-dmg.sh
