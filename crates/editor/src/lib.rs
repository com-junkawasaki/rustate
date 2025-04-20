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

// WASMエントリーポイント
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    set_panic_hook();
    Ok(())
}

#[wasm_bindgen]
pub fn init_editor(container_id: &str) -> Result<(), JsValue> {
    yew::Renderer::<components::App>::with_root(
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(container_id)
            .unwrap(),
    )
    .render();
    Ok(())
}