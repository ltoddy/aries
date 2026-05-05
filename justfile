nightly_toolchain := "nightly"

install:
    @cargo install --path crates/aries-cli

build:
    @cargo build --all

check:
    @cargo check --all

test:
    @cargo test

clippy:
    @cargo clippy

fmt:
    @cargo +{{nightly_toolchain}} fmt

fmt-check:
    @cargo +{{nightly_toolchain}} fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty --all
    @cargo +{{nightly_toolchain}} fmt

lint: fmt-check clippy

ui-install:
    @cd crates/aries-ui/app && npm install

ui-dev:
    @cd crates/aries-ui/app && npm run dev

ui-build:
    @cd crates/aries-ui/app && npm run build

ui-tauri-dev:
    @cd crates/aries-ui && cargo tauri dev

ui-tauri-build:
    @cd crates/aries-ui/app && npm run build
    @cd crates/aries-ui && cargo tauri build

ui-macos-package:
    @cd crates/aries-ui/app && npm run build
    @cd crates/aries-ui && cargo tauri build --bundles app,dmg
