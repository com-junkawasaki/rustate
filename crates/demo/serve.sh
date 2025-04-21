#!/bin/bash

# Check if Trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "Trunk is not installed. Installing now..."
    cargo install trunk
fi

# Check if wasm32-unknown-unknown target is installed
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "wasm32-unknown-unknown target not installed. Installing now..."
    rustup target add wasm32-unknown-unknown
fi

# Set the working directory
cd "$(dirname "$0")"

# Build and serve the application
echo "Starting Trunk server..."
trunk serve 