use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // Use `web_sys`'s global `window` function to get a handle on the global window object.
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    
    // Log a message to the console
    web_sys::console::log_1(&"RuState WebAssembly app initialized!".into());
    
    // Re-export the rustate library functionality
    // This will make the rustate functionality available to the JS code
    Ok(())
}

// Re-export the rustate library
pub use rustate::*; 