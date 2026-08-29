OUTPUTS = src-tauri/gen/android/app/build/outputs/apk
PKG = dev.kerker.slowshow

# Dateinamen werden gesucht, nicht geraten: ohne Signaturschluessel heisst das
# Release-APK "...-release-unsigned.apk" und laesst sich nicht installieren.
APK_RELEASE = $(shell find $(OUTPUTS) -name "app-universal-release.apk" 2>/dev/null | head -1)
APK_DEBUG   = $(shell find $(OUTPUTS) -name "app-universal-debug.apk" 2>/dev/null | head -1)

.PHONY: init patch deploy deploy-debug build-android build-desktop test check

init:
	npx tauri android init
	node scripts/patch-android.mjs

patch:
	node scripts/patch-android.mjs

# Zum Testen ist deploy-debug der richtige Weg (siehe docs/signing.md).
deploy: patch
	npx tauri android build --apk
	@test -n "$(APK_RELEASE)" || (echo "Kein signiertes Release-APK - siehe docs/signing.md"; exit 1)
	adb install -r $(APK_RELEASE)
	adb shell am start -n "$(PKG)/.MainActivity"

deploy-debug: patch
	npx tauri android build --apk --debug
	adb install -r $(APK_DEBUG)
	adb shell am start -n "$(PKG)/.MainActivity"

build-android: patch
	npx tauri android build

build-desktop:
	npx tauri build

test:
	npm run test:run
	cd src-tauri && cargo test

check:
	npx vue-tsc --noEmit
	cd src-tauri && cargo check
