#!/bin/sh
# Builds the Rust release binary and assembles dist/Panto Studio.app
# (same layout `cargo tauri build` produces; useful without the Tauri CLI).
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/dist/Panto Studio.app"

cargo build --release --offline --manifest-path "$ROOT/src-tauri/Cargo.toml"

pkill -x panto-studio 2>/dev/null || true
sleep 1
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/whisper/lib"
cp "$ROOT/src-tauri/target/release/panto-studio" "$APP/Contents/MacOS/panto-studio"
cp "$ROOT/assets/icons/panto-srt.icns" "$APP/Contents/Resources/icon.icns"
cp "$ROOT/build/whisper/whisper-cli" "$APP/Contents/Resources/whisper/whisper-cli"
if [ -d "$ROOT/build/whisper/lib" ]; then
  cp -R "$ROOT/build/whisper/lib/." "$APP/Contents/Resources/whisper/lib/"
fi
if [ -d "$ROOT/build/nemo" ]; then
  cp -R "$ROOT/build/nemo" "$APP/Contents/Resources/nemo"
fi
cp "$ROOT/THIRD_PARTY_NOTICES.md" "$APP/Contents/Resources/"
chmod +x "$APP/Contents/MacOS/panto-studio" \
  "$APP/Contents/Resources/whisper/whisper-cli"
if [ -x "$APP/Contents/Resources/nemo/bin/nemo-speech" ]; then
  chmod +x "$APP/Contents/Resources/nemo/bin/nemo-speech"
fi

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>English</string>
  <key>CFBundleDisplayName</key><string>Panto Studio</string>
  <key>CFBundleExecutable</key><string>panto-studio</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundleIdentifier</key><string>com.panto.studio</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Panto Studio</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>13.4</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
echo "✓ $APP"
