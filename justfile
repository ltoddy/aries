install-cli:
    @cargo +stable install --path apps/aries-cli --force --locked

install-orbit:
    @cargo +stable install --path apps/aries-orbit --force --locked

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

# Aries Orbit macOS packaging
orbit-dmg:
    @cargo build --release --locked --bin aries-orbit
    @VERSION=$$(cargo metadata --no-deps --format-version 1 --locked | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])'); \
    BINARY_PATH="target/release/aries-orbit"; \
    APP_BUNDLE="target/bundle/Aries Orbit.app"; \
    INFO_PLIST="$$APP_BUNDLE/Contents/Info.plist"; \
    rm -rf "$$APP_BUNDLE"; \
    mkdir -p "$$APP_BUNDLE/Contents/MacOS"; \
    install -m 755 "$$BINARY_PATH" "$$APP_BUNDLE/Contents/MacOS/aries-orbit"; \
    plutil -create xml1 "$$INFO_PLIST"; \
    plutil -insert CFBundleName -string "Aries Orbit" "$$INFO_PLIST"; \
    plutil -insert CFBundleDisplayName -string "Aries Orbit" "$$INFO_PLIST"; \
    plutil -insert CFBundleExecutable -string "aries-orbit" "$$INFO_PLIST"; \
    plutil -insert CFBundleIdentifier -string "com.ltoddy.aries-orbit" "$$INFO_PLIST"; \
    plutil -insert CFBundlePackageType -string APPL "$$INFO_PLIST"; \
    plutil -insert CFBundleInfoDictionaryVersion -string "6.0" "$$INFO_PLIST"; \
    plutil -insert CFBundleShortVersionString -string "$$VERSION" "$$INFO_PLIST"; \
    plutil -insert CFBundleVersion -string "$$VERSION" "$$INFO_PLIST"; \
    plutil -insert LSMinimumSystemVersion -string "$$(xcrun vtool -show-build "$$BINARY_PATH" | awk '$$1 == "minos" { print $$2; exit }')" "$$INFO_PLIST"; \
    plutil -insert LSApplicationCategoryType -string public.app-category.developer-tools "$$INFO_PLIST"; \
    plutil -insert NSHighResolutionCapable -bool true "$$INFO_PLIST"; \
    plutil -insert NSSupportsAutomaticGraphicsSwitching -bool true "$$INFO_PLIST"; \
    printf 'APPL????' > "$$APP_BUNDLE/Contents/PkgInfo"; \
    plutil -lint "$$INFO_PLIST"; \
    codesign --force --sign - "$$APP_BUNDLE"; \
    codesign --verify --strict --verbose=2 "$$APP_BUNDLE"; \
    DMG_PATH="target/bundle/Aries-Orbit-$$VERSION.dmg"; \
    stage=$$(mktemp -d "target/bundle/.dmg.XXXXXX"); \
    trap 'rm -rf "$$stage"' EXIT; \
    ditto "$$APP_BUNDLE" "$$stage/Aries Orbit.app"; \
    ln -s /Applications "$$stage/Applications"; \
    hdiutil create -volname "Aries Orbit" -srcfolder "$$stage" -fs HFS+ -format UDZO -ov "$$DMG_PATH"; \
    hdiutil verify "$$DMG_PATH"; \
    @printf 'DMG: %s\\n' "$$DMG_PATH"
