use wasm_bindgen::prelude::*;

// mainモジュールを公開
pub mod main;

// Wasm用のパニックフックを設定
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(start)]
pub fn run() {
    init_panic_hook();
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<main::App>::new().render();
}