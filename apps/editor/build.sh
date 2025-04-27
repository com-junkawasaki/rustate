#!/bin/bash
set -e

echo "Building rustate-editor WASM package..."

# WASMにビルド
wasm-pack build --target web --out-dir www/pkg

echo "Build completed successfully!"
echo "To serve the app, run:"
echo "cd www && python3 -m http.server"