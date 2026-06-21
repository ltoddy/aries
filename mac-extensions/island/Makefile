PROJECT      := Island.xcodeproj
SCHEME       := Island
CONFIG       := Release
APP_NAME     := Aries Island.app
DERIVED_DATA := $(HOME)/Library/Developer/Xcode/DerivedData/Island-Makefile
PRODUCT_DIR  := $(DERIVED_DATA)/Build/Products/$(CONFIG)
APP_PATH     := $(PRODUCT_DIR)/$(APP_NAME)
INSTALL_DIR  ?= $(HOME)/Applications
INSTALL_PATH := $(INSTALL_DIR)/$(APP_NAME)
ARCH         := $(shell uname -m)
XCODEBUILD_FLAGS ?= ONLY_ACTIVE_ARCH=YES CODE_SIGN_STYLE=Manual DEVELOPMENT_TEAM= CODE_SIGN_IDENTITY=- CODE_SIGNING_ALLOWED=YES

.PHONY: install uninstall run clean

install:
	@xcodebuild -project "$(PROJECT)" -scheme "$(SCHEME)" -configuration "$(CONFIG)" \
		-derivedDataPath "$(DERIVED_DATA)" -destination "platform=macOS,arch=$(ARCH)" build $(XCODEBUILD_FLAGS)
	@mkdir -p "$(INSTALL_DIR)"
	@rm -rf "$(INSTALL_PATH)"
	@ditto "$(APP_PATH)" "$(INSTALL_PATH)"
	@rm -rf "$(APP_PATH)"
	@echo "Installed: $(INSTALL_PATH)"

uninstall:
	@rm -rf "$(INSTALL_PATH)"
	@echo "Removed: $(INSTALL_PATH)"

run:
	@open "$(INSTALL_PATH)"

clean:
	@rm -rf "$(DERIVED_DATA)"
	@echo "Cleaned build artifacts"
