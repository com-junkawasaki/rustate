use crate::editor::EditorState;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct EditorPanelProps {
    pub editor_state: UseStateHandle<EditorState>,
}

#[function_component(EditorPanel)]
pub fn editor_panel(props: &EditorPanelProps) -> Html {
    let json_text = use_state(|| String::new());
    
    let on_json_change = {
        let json_text = json_text.clone();
        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<web_sys::HtmlTextAreaElement>().unwrap();
            json_text.set(input.value());
        })
    };
    
    let on_load_json = {
        let json_text = json_text.clone();
        let editor_state = props.editor_state.clone();
        
        Callback::from(move |_: MouseEvent| {
            match serde_json::from_str::<rustate::machine::StateMachine>(&json_text) {
                Ok(machine) => {
                    editor_state.set(EditorState {
                        machine,
                        selected_element: None,
                        ..(*editor_state).clone()
                    });
                }
                Err(err) => {
                    web_sys::console::error_1(&format!("JSON解析エラー: {}", err).into());
                }
            }
        })
    };
    
    let on_export_json = {
        let editor_state = props.editor_state.clone();
        let json_text = json_text.clone();
        
        Callback::from(move |_: MouseEvent| {
            match serde_json::to_string_pretty(&editor_state.machine) {
                Ok(json) => {
                    json_text.set(json);
                }
                Err(err) => {
                    web_sys::console::error_1(&format!("JSON出力エラー: {}", err).into());
                }
            }
        })
    };

    html! {
        <div class="editor-panel">
            <h3>{"JSONエディタ"}</h3>
            <textarea
                value={(*json_text).clone()}
                onchange={on_json_change}
                rows="20"
                cols="40"
                placeholder="ステートマシンのJSONを入力または出力されます"
            ></textarea>
            <div class="button-group">
                <button onclick={on_load_json}>{"JSONを読み込む"}</button>
                <button onclick={on_export_json}>{"JSONを出力"}</button>
            </div>
        </div>
    }
}