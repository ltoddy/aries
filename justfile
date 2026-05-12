install:
    @cargo install --path crates/aries-cli

run:
    @cargo run

build:
    @cargo build

check:
    @cargo check

clippy:
    @cargo clippy

fmt:
    @cargo +nightly fmt

fmt-check:
    @cargo +nightly fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty
    @cargo +nightly fmt

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
