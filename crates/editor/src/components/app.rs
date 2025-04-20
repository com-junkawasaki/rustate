use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    // シンプルな初期表示用コンポーネント
    html! {
        <div class="editor-app" style="padding: 20px;">
            <h2>{"Rustate ステートマシンエディタ"}</h2>
            <p>{"WASMが正常にロードされました。"}</p>
            <div style="display: flex; gap: 10px;">
                <button style="padding: 8px 16px; background-color: #4CAF50; color: white; border: none; border-radius: 4px; cursor: pointer;">
                    {"+ ステート追加"}
                </button>
                <button style="padding: 8px 16px; background-color: #4CAF50; color: white; border: none; border-radius: 4px; cursor: pointer;">
                    {"+ 避難追加"}
                </button>
            </div>
            <div style="margin-top: 20px; height: 400px; border: 1px dashed #ccc; display: flex; justify-content: center; align-items: center;">
                <p>{"(キャンバス領域 - 実装中)"}</p>
            </div>
        </div>
    }
}