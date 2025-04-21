#!/bin/bash
set -e

echo "Rebuilding RuState WebAssembly module..."

# Navigate to the rustate crate
cd ../crates/rustate

# Clean previous builds
rm -rf pkg || true

# Build with wasm-pack using web target and wasm features
wasm-pack build --target web --features wasm

# Copy to web/pkg
echo "Copying built files to web/pkg directory..."
mkdir -p ../../web/pkg
cp -r pkg/* ../../web/pkg/

echo "Build completed. Refresh your browser to see the changes." 