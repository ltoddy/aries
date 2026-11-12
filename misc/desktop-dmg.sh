#!/bin/bash
set -euo pipefail

VERSION=$(cargo metadata --no-deps --format-version 1 --locked | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')

BINARY_PATH="target/release/aries-desktop"
APP_BUNDLE="target/bundle/Aries Desktop.app"
INFO_PLIST="$APP_BUNDLE/Contents/Info.plist"

rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources"
install -m 755 "$BINARY_PATH" "$APP_BUNDLE/Contents/MacOS/aries-desktop"
install -m 644 "apps/aries-desktop/assets/aries.icns" "$APP_BUNDLE/Contents/Resources/aries.icns"

plutil -create xml1 "$INFO_PLIST"
plutil -insert CFBundleName -string "Aries Desktop" "$INFO_PLIST"
plutil -insert CFBundleDisplayName -string "Aries Desktop" "$INFO_PLIST"
plutil -insert CFBundleExecutable -string "aries-desktop" "$INFO_PLIST"
plutil -insert CFBundleIconFile -string "aries.icns" "$INFO_PLIST"
plutil -insert CFBundleIdentifier -string "com.ltoddy.aries-desktop" "$INFO_PLIST"
plutil -insert CFBundlePackageType -string APPL "$INFO_PLIST"
plutil -insert CFBundleInfoDictionaryVersion -string "6.0" "$INFO_PLIST"
plutil -insert CFBundleShortVersionString -string "$VERSION" "$INFO_PLIST"
plutil -insert CFBundleVersion -string "$VERSION" "$INFO_PLIST"

MINOS=$(xcrun vtool -show-build "$BINARY_PATH" | awk '$1 == "minos" { print $2; exit }')
plutil -insert LSMinimumSystemVersion -string "$MINOS" "$INFO_PLIST"
plutil -insert LSApplicationCategoryType -string public.app-category.developer-tools "$INFO_PLIST"
plutil -insert NSHighResolutionCapable -bool true "$INFO_PLIST"
plutil -insert NSSupportsAutomaticGraphicsSwitching -bool true "$INFO_PLIST"

printf 'APPL????' > "$APP_BUNDLE/Contents/PkgInfo"
plutil -lint "$INFO_PLIST"

codesign --force --sign - "$APP_BUNDLE"
codesign --verify --strict --verbose=2 "$APP_BUNDLE"

DMG_PATH="target/bundle/Aries-Desktop-$VERSION.dmg"

stage=$(mktemp -d "target/bundle/.dmg.XXXXXX")
trap 'rm -rf "$stage"' EXIT

ditto "$APP_BUNDLE" "$stage/Aries Desktop.app"
ln -s /Applications "$stage/Applications"

hdiutil create -volname "Aries Desktop" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$DMG_PATH"
hdiutil verify "$DMG_PATH"

printf 'DMG: %s\n' "$DMG_PATH"