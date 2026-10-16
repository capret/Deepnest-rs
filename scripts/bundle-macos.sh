#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
case "${1:-release}" in
  release) cargo build --release -p deepnest-app; profile=release ;;
  --debug) cargo build -p deepnest-app; profile=debug ;;
  *) echo "Usage: bash scripts/bundle-macos.sh [--debug]" >&2; exit 1 ;;
esac

bundle="target/$profile/Deepnest.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "target/$profile/deepnest" "$bundle/Contents/MacOS/deepnest"
cp resources/macos/Info.plist "$bundle/Contents/Info.plist"
cp resources/macos/icon.icns "$bundle/Contents/Resources/icon.icns"
echo "Built $bundle"
