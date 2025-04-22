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

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    // WASM用テストの設定
    wasm_bindgen_test_configure!(run_in_browser);

    // モックDOM要素を作成するヘルパー
    fn create_mock_element() -> web_sys::Element {
        let window = web_sys::window().expect("no global window exists");
        let document = window.document().expect("no document exists");
        document
            .create_element("div")
            .expect("could not create element")
    }

    #[wasm_bindgen_test]
    fn test_editor_initialization() {
        // モック要素を作成
        let container = create_mock_element();

        // エディタの初期化オプション
        let options = EditorOptions {
            theme: "light".to_string(),
            show_toolbar: true,
            read_only: false,
            auto_save: true,
        };

        // エディタの初期化
        let editor = Editor::new(options);

        // 属性の確認
        assert_eq!(editor.options.theme, "light");
        assert!(editor.options.show_toolbar);
        assert!(!editor.options.read_only);
        assert!(editor.options.auto_save);

        // 初期状態の確認
        assert!(editor.layout.is_none());
        assert!(editor.components.is_empty());
    }

    #[wasm_bindgen_test]
    fn test_editor_with_layout() {
        // エディタインスタンスを作成
        let options = EditorOptions {
            theme: "dark".to_string(),
            show_toolbar: true,
            read_only: false,
            auto_save: true,
        };
        let mut editor = Editor::new(options);

        // レイアウトを設定
        let layout = layout::Layout::new("test-layout");
        editor.set_layout(layout);

        // レイアウトが設定されたことを確認
        assert!(editor.layout.is_some());
        assert_eq!(editor.layout.as_ref().unwrap().id, "test-layout");
    }

    #[wasm_bindgen_test]
    fn test_add_component() {
        // エディタインスタンスを作成
        let options = EditorOptions::default();
        let mut editor = Editor::new(options);

        // コンポーネントを作成
        let component_id = "test-component";

        // コンポーネント追加前の確認
        assert_eq!(editor.components.len(), 0);

        // コンポーネントを追加
        editor.add_component(
            component_id.to_string(),
            Box::new(MockComponent::new(component_id)),
        );

        // コンポーネントが追加されたことを確認
        assert_eq!(editor.components.len(), 1);
        assert!(editor.components.contains_key(component_id));
    }

    #[wasm_bindgen_test]
    fn test_remove_component() {
        // エディタインスタンスを作成
        let options = EditorOptions::default();
        let mut editor = Editor::new(options);

        // コンポーネントを追加
        let component_id = "test-component";
        editor.add_component(
            component_id.to_string(),
            Box::new(MockComponent::new(component_id)),
        );

        // 追加後の確認
        assert_eq!(editor.components.len(), 1);

        // コンポーネントを削除
        editor.remove_component(component_id);

        // 削除後の確認
        assert_eq!(editor.components.len(), 0);
        assert!(!editor.components.contains_key(component_id));
    }

    #[wasm_bindgen_test]
    fn test_editor_render() {
        // エディタインスタンスを作成
        let options = EditorOptions::default();
        let mut editor = Editor::new(options);

        // コンポーネントを追加
        let component_id = "test-component";
        editor.add_component(
            component_id.to_string(),
            Box::new(MockComponent::new(component_id)),
        );

        // レイアウトを設定
        let layout = layout::Layout::new("test-layout");
        editor.set_layout(layout);

        // コンテナ要素を作成
        let container = create_mock_element();

        // エディタをレンダリング（エラーが発生しないことを確認）
        let result = std::panic::catch_unwind(|| {
            editor.render(&container);
        });

        assert!(!result.is_err(), "Editor rendering panicked");
    }

    #[wasm_bindgen_test]
    fn test_editor_event_handlers() {
        // エディタインスタンスを作成
        let options = EditorOptions::default();
        let mut editor = Editor::new(options);

        // モックイベントハンドラのカウンタ
        let counter = std::rc::Rc::new(std::cell::RefCell::new(0));
        let counter_clone = counter.clone();

        // イベントハンドラを設定
        editor.on_save(Box::new(move || {
            let mut count = counter_clone.borrow_mut();
            *count += 1;
        }));

        // イベント発火
        editor.trigger_save();

        // ハンドラが呼び出されたことを確認
        assert_eq!(*counter.borrow(), 1);

        // 再度イベント発火
        editor.trigger_save();

        // 再度ハンドラが呼び出されたことを確認
        assert_eq!(*counter.borrow(), 2);
    }

    // テスト用モックコンポーネント
    struct MockComponent {
        id: String,
    }

    impl MockComponent {
        fn new(id: &str) -> Self {
            Self { id: id.to_string() }
        }
    }

    impl Component for MockComponent {
        fn id(&self) -> &str {
            &self.id
        }

        fn render(&self, _container: &web_sys::Element) -> Result<(), JsValue> {
            // レンダリングの実装（テスト用にダミー）
            Ok(())
        }

        fn update(&self) -> Result<(), JsValue> {
            // 更新の実装（テスト用にダミー）
            Ok(())
        }
    }
}
