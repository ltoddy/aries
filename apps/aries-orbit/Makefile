.DEFAULT_GOAL := dmg

CARGO ?= cargo
CARGO_TARGET_DIR ?= target
TARGET ?= $(CARGO_BUILD_TARGET)
SIGN_IDENTITY ?= -

APP_NAME := Aries Orbit
BINARY_NAME := aries-orbit
BUNDLE_ID := com.ltoddy.aries-orbit
VERSION = $(shell $(CARGO) metadata --no-deps --format-version 1 --locked | plutil -extract packages.0.version raw -o - -)
BUNDLE_DIR = $(CARGO_TARGET_DIR)/bundle
APP_BUNDLE = $(BUNDLE_DIR)/$(APP_NAME).app
INFO_PLIST = $(APP_BUNDLE)/Contents/Info.plist
BINARY_PATH = $(CARGO_TARGET_DIR)/$(if $(TARGET),$(TARGET)/)release/$(BINARY_NAME)
DMG_PATH = $(BUNDLE_DIR)/Aries-Orbit-$(VERSION)$(if $(TARGET),-$(TARGET)).dmg

.PHONY: help check-macos build app dmg clean

help:
	@printf '%s\n' 'make / make dmg  构建 Release 应用和 DMG 安装包' 'make app         构建 macOS .app' 'make build       构建 Release 可执行文件' 'make clean       清理 target/bundle 中的打包产物'

check-macos:
	@test "$$(uname -s)" = Darwin || { printf '%s\n' '请在 macOS 上构建安装包。' >&2; exit 1; }
	@test -n "$(CARGO_TARGET_DIR)" || { printf '%s\n' 'CARGO_TARGET_DIR 不能为空。' >&2; exit 1; }
	@case "$(TARGET)" in ''|aarch64-apple-darwin|x86_64-apple-darwin) ;; *) printf '%s\n' 'TARGET 必须是 macOS Rust target。' >&2; exit 1 ;; esac

build: check-macos
	$(CARGO) build --release --locked --bin "$(BINARY_NAME)" --target-dir "$(CARGO_TARGET_DIR)" $(if $(TARGET),--target "$(TARGET)")

app: build
	@test -n "$(VERSION)" || { printf '%s\n' '无法读取 Cargo.toml 中的应用版本。' >&2; exit 1; }
	rm -rf "$(APP_BUNDLE)"
	mkdir -p "$(APP_BUNDLE)/Contents/MacOS" "$(APP_BUNDLE)/Contents/Resources/bin"
	install -m 755 "$(BINARY_PATH)" "$(APP_BUNDLE)/Contents/MacOS/$(BINARY_NAME)"
	install -m 755 "scripts/aries-orbit" "$(APP_BUNDLE)/Contents/Resources/bin/$(BINARY_NAME)"
	plutil -create xml1 "$(INFO_PLIST)"
	plutil -insert CFBundleName -string "$(APP_NAME)" "$(INFO_PLIST)"
	plutil -insert CFBundleDisplayName -string "$(APP_NAME)" "$(INFO_PLIST)"
	plutil -insert CFBundleExecutable -string "$(BINARY_NAME)" "$(INFO_PLIST)"
	plutil -insert CFBundleIdentifier -string "$(BUNDLE_ID)" "$(INFO_PLIST)"
	plutil -insert CFBundlePackageType -string APPL "$(INFO_PLIST)"
	plutil -insert CFBundleInfoDictionaryVersion -string 6.0 "$(INFO_PLIST)"
	plutil -insert CFBundleShortVersionString -string "$(VERSION)" "$(INFO_PLIST)"
	plutil -insert CFBundleVersion -string "$(VERSION)" "$(INFO_PLIST)"
	plutil -insert LSMinimumSystemVersion -string "$$(xcrun vtool -show-build "$(BINARY_PATH)" | awk '$$1 == "minos" { print $$2; exit }')" "$(INFO_PLIST)"
	plutil -insert LSApplicationCategoryType -string public.app-category.developer-tools "$(INFO_PLIST)"
	plutil -insert NSHighResolutionCapable -bool true "$(INFO_PLIST)"
	plutil -insert NSSupportsAutomaticGraphicsSwitching -bool true "$(INFO_PLIST)"
	printf 'APPL????' > "$(APP_BUNDLE)/Contents/PkgInfo"
	plutil -lint "$(INFO_PLIST)"
	codesign --force --sign "$(SIGN_IDENTITY)" $(if $(filter -,$(SIGN_IDENTITY)),,--options runtime --timestamp) "$(APP_BUNDLE)"
	codesign --verify --strict --verbose=2 "$(APP_BUNDLE)"
	@printf 'App: %s\n' "$(APP_BUNDLE)"

dmg: app
	@set -eu; \
	stage=$$(mktemp -d "$(BUNDLE_DIR)/.dmg.XXXXXX"); \
	trap 'rm -rf "$$stage"' EXIT; \
	ditto "$(APP_BUNDLE)" "$$stage/$(APP_NAME).app"; \
	ln -s /Applications "$$stage/Applications"; \
	hdiutil create -volname "$(APP_NAME)" -srcfolder "$$stage" -fs HFS+ -format UDZO -ov "$(DMG_PATH)"
	hdiutil verify "$(DMG_PATH)"
	@printf 'DMG: %s\n' "$(DMG_PATH)"

clean:
	@test -n "$(CARGO_TARGET_DIR)"
	rm -rf "$(BUNDLE_DIR)"
