mod components;
mod editor;
mod layout;
mod utils;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// パニックハンドラの設定
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// エディタモジュールの公開
pub use editor::Editor;

// デバッグ用ヘルパー関数
#[wasm_bindgen]
pub fn debug_log(message: &str) {
    web_sys::console::log_1(&JsValue::from_str(message));
}

// WASMエントリーポイント
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    set_panic_hook();
    web_sys::console::log_1(&JsValue::from_str("Rustate Editor WASM started"));
    Ok(())
}

#[wasm_bindgen]
pub fn init_editor(container_id: &str) -> Result<(), JsValue> {
    web_sys::console::log_1(&JsValue::from_str(&format!(
        "Initializing editor in container: {}",
        container_id
    )));

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Window not found"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("Document not found"))?;
    let container = document.get_element_by_id(container_id).ok_or_else(|| {
        JsValue::from_str(&format!("Container element not found: {}", container_id))
    })?;

    web_sys::console::log_1(&JsValue::from_str("Rendering Yew app"));
    yew::Renderer::<components::App>::with_root(container).render();

    web_sys::console::log_1(&JsValue::from_str("Editor initialized successfully"));
    Ok(())
}
