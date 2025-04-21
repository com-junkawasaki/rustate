use crate::editor::EditorState;
use rustate::state::State;
use rustate::transition::Transition;
use serde_json::json;
use uuid::Uuid;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ToolbarProps {
    pub editor_state: UseStateHandle<EditorState>,
}

#[function_component(Toolbar)]
pub fn toolbar(props: &ToolbarProps) -> Html {
    let mode = use_state(|| "select".to_string());
    
    let on_add_state = {
        let editor_state = props.editor_state.clone();
        
        Callback::from(move |_: MouseEvent| {
            let mut new_editor_state = (*editor_state).clone();
            let state_id = Uuid::new_v4().to_string();
            
            let new_state = State::new(&state_id, rustate::state::StateType::Normal);
            
            new_editor_state.machine.states.insert(state_id, new_state);
            editor_state.set(new_editor_state);
        })
    };
    
    let on_add_transition = {
        let editor_state = props.editor_state.clone();
        let mode = mode.clone();
        
        Callback::from(move |_: MouseEvent| {
            if *mode == "add_transition" {
                mode.set("select".to_string());
            } else {
                mode.set("add_transition".to_string());
            }
        })
    };

    let on_delete_element = {
        let editor_state = props.editor_state.clone();
        
        Callback::from(move |_: MouseEvent| {
            if let Some(element_id) = &editor_state.selected_element {
                let mut new_editor_state = (*editor_state).clone();
                
                if new_editor_state.machine.states.contains_key(element_id) {
                    new_editor_state.machine.states.remove(element_id);
                    
                    new_editor_state.machine.transitions.retain(|t| {
                        t.source != *element_id && t.target.as_ref().map_or(true, |target| target != element_id)
                    });
                } else {
                    let transition_index = new_editor_state.machine.transitions.iter()
                        .position(|t| {
                            format!("{}-{}-{}", t.source, t.target.as_ref().unwrap_or(&String::new()),
                                t.event.as_ref().unwrap_or(&String::new())) == *element_id
                        });
                        
                    if let Some(index) = transition_index {
                        new_editor_state.machine.transitions.remove(index);
                    }
                }
                
                new_editor_state.selected_element = None;
                editor_state.set(new_editor_state);
            }
        })
    };

    let on_generate_code = {
        let editor_state = props.editor_state.clone();
        
        Callback::from(move |_: MouseEvent| {
            web_sys::window()
                .and_then(|win| win.open_with_url_and_target("/generate-code", "_blank").ok())
                .expect("Failed to open code generation tab");
        })
    };

    html! {
        <div class="toolbar">
            <div class="button-group">
                <button onclick={on_add_state}>{"+ ステート追加"}</button>
                <button 
                    onclick={on_add_transition} 
                    class={if *mode == "add_transition" { "active" } else { "" }}
                >
                    {"+ 遷移追加"}
                </button>
                <button 
                    onclick={on_delete_element}
                    disabled={props.editor_state.selected_element.is_none()}
                >
                    {"削除"}
                </button>
            </div>
            <div class="button-group">
                <button onclick={on_generate_code}>{"Rustコード生成"}</button>
            </div>
        </div>
    }
}