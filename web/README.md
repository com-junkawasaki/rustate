# RuState WebAssembly Demo

This is the WebAssembly demo page for the RuState library. You can see how the traffic light and music player state machines work in your browser.

## How to Build

### Required Tools

- Rust (1.50 or later)
- wasm-pack (0.9.1 or later)
- Node.js (14.0.0 or later)

### Installation

```bash
# Install wasm-pack
cargo install wasm-pack

# Other dependencies
npm install
```

### Build Steps

```bash
# Compile the rustate crate to wasm
wasm-pack build --target web --features wasm

# Start the development server
cd web
python -m http.server
# Or
npx serve
```

## How to Run

After building, access http://localhost:8000 or the URL displayed by the development server in your browser.

## Demo Contents

### Traffic Light

A simple 3-state traffic light. When you click the "Send Timer Event" button, the following transitions occur:

```
green → yellow → red → green → ...
```

### Music Player

A music player demo using a hierarchical state machine. It has the following states and transitions:

- Power off/on states
- Play/stop/pause states
- Normal speed/high speed playback states
- Track change functionality

## Troubleshooting

If you encounter issues, check the following:

1. Make sure Rust and wasm-pack are up to date
2. Check if there are any error messages in the console
3. Verify that the `pkg` directory has been generated correctly

## Notes

- This demo is for educational purposes and does not include actual music playback functionality
- A modern browser compatible with WebAssembly is required 