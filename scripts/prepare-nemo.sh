#!/bin/sh
# Downloads the NeMo-Speech.cpp runtime (Parakeet engine) into build/nemo/.
# macOS Apple Silicon (Metal) only for now; mirrors prepare-whisper.js.
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/build/nemo"
VERSION="0.3.0"
ASSET="nemo-speech-${VERSION}-macos-aarch64-metal.tar.gz"
URL="https://github.com/NVIDIA/NeMo-Speech.cpp/releases/download/v${VERSION}/${ASSET}"

if [ -x "$DEST/bin/nemo-speech" ]; then
  echo "✓ Motor NeMo ya presente: build/nemo/bin/nemo-speech"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
echo "Descargando NeMo-Speech.cpp ${VERSION}…"
curl -fsSL -o "$TMP/$ASSET" "$URL"
mkdir -p "$DEST"
tar -xzf "$TMP/$ASSET" -C "$TMP"
cp -R "$TMP/nemo-speech-${VERSION}-macos-aarch64-metal/bin" "$DEST/"
cp -R "$TMP/nemo-speech-${VERSION}-macos-aarch64-metal/lib" "$DEST/"
chmod +x "$DEST/bin/nemo-speech"
echo "✓ Motor NeMo listo: build/nemo/bin/nemo-speech"
