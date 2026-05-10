install:
    @cargo install --path crates/aries-cli -Zbuild-dir-new-layout

run:
    @cargo run -Zbuild-dir-new-layout


build:
    @cargo build --all -Zbuild-dir-new-layout

check:
    @cargo check --all -Zbuild-dir-new-layout

clippy:
    @cargo clippy -Zbuild-dir-new-layout

fmt:
    @cargo fmt

fmt-check:
    @cargo fmt -- --check

fix:
    @cargo clippy --fix --allow-dirty --all
    @cargo fmt

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
