# RuState Web Demo

This directory contains a WebAssembly demo for the RuState statechart library.

## Prerequisites

- Rust (1.56.0 or later)
- [Trunk](https://trunkrs.dev/) - WebAssembly bundler for Rust
- wasm-bindgen-cli - WebAssembly binding generator

## Installation

If you don't have Trunk installed:

```
cargo install trunk
```

If you don't have wasm-bindgen-cli installed:

```
cargo install wasm-bindgen-cli
```

Make sure the wasm32-unknown-unknown target is installed:

```
rustup target add wasm32-unknown-unknown
```

## Building and Running

### Development Mode

To run the development server:

```
trunk serve
```

This will:
1. Compile the Rust code to WebAssembly
2. Generate the JavaScript bindings
3. Bundle the assets
4. Start a development server at http://localhost:8080

### Production Build

To create a production build:

```
trunk build --release
```

The output will be in the `dist` directory.

## Project Structure

- `src/` - Rust source code
  - `lib.rs` - Main WebAssembly library that exposes the RuState functionality to JavaScript
  - `main.rs` - Application entry point

- `static/` - Static assets
- `Trunk.toml` - Trunk configuration
- `index.html` - Main HTML entry point
- `style.css` - CSS styles

## Rebuilding rustate WASM module

If you've made changes to the RuState library and want to update the web demo:

```
./rebuild-wasm.sh
```

This script will rebuild the RuState library with WebAssembly support and copy the generated files to the web/pkg directory.

## Troubleshooting

If you encounter any issues:

1. Make sure you have the latest version of Trunk and wasm-bindgen-cli
2. Try clearing the Trunk cache: `trunk clean`
3. If you modified the RuState library, make sure to run `./rebuild-wasm.sh`
4. Check the browser console for any JavaScript errors
5. Verify that the WebAssembly module is loaded correctly

## Resources

- [Trunk Documentation](https://trunkrs.dev/)
- [wasm-bindgen Documentation](https://rustwasm.github.io/docs/wasm-bindgen/)
- [RuState Documentation](https://docs.rs/rustate) 