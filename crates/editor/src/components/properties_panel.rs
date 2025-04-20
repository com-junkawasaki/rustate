use crate::editor::EditorState;
use yew::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Properties, PartialEq)]
pub struct PropertiesPanelProps {
    pub editor_state: UseStateHandle<EditorState>,
}

#[function_component(PropertiesPanel)]
pub fn properties_panel(props: &PropertiesPanelProps) -> Html {
    let editor_state = &*props.editor_state;
    
    let render_state_properties = {
        let editor_state = props.editor_state.clone();
        
        if let Some(element_id) = &editor_state.selected_element {
            if let Some(state) = editor_state.machine.states.get(element_id) {
                let name = state.name.clone();
                let is_final = state.is_final;
                
                let on_name_change = {
                    let editor_state = editor_state.clone();
                    let element_id = element_id.clone();
                    
                    Callback::from(move |e: Event| {
                        let target = e.target().unwrap();
                        let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
                        let mut new_state = (*editor_state).clone();
                        
                        if let Some(state) = new_state.machine.states.get_mut(&element_id) {
                            state.name = input.value();
                            editor_state.set(new_state);
                        }
                    })
                };
                
                let on_final_change = {
                    let editor_state = editor_state.clone();
                    let element_id = element_id.clone();
                    
                    Callback::from(move |e: Event| {
                        let target = e.target().unwrap();
                        let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
                        let mut new_state = (*editor_state).clone();
                        
                        if let Some(state) = new_state.machine.states.get_mut(&element_id) {
                            state.is_final = input.checked();
                            editor_state.set(new_state);
                        }
                    })
                };
                
                return html! {
                    <div class="state-properties">
                        <h3>{"ステートプロパティ"}</h3>
                        <div class="property-group">
                            <label>{"名前:"}</label>
                            <input type="text" value={name} onchange={on_name_change} />
                        </div>
                        <div class="property-group">
                            <label>{"終了ステート:"}</label>
                            <input type="checkbox" checked={is_final} onchange={on_final_change} />
                        </div>
                    </div>
                };
            } else if let Some(transition) = editor_state.machine.transitions.get(element_id) {
                let event = transition.event.clone().unwrap_or_default();
                let source = transition.source.clone();
                let target = transition.target.clone();
                
                let on_event_change = {
                    let editor_state = editor_state.clone();
                    let element_id = element_id.clone();
                    
                    Callback::from(move |e: Event| {
                        let target = e.target().unwrap();
                        let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
                        let mut new_state = (*editor_state).clone();
                        
                        if let Some(transition) = new_state.machine.transitions.get_mut(&element_id) {
                            transition.event = Some(input.value());
                            editor_state.set(new_state);
                        }
                    })
                };
                
                return html! {
                    <div class="transition-properties">
                        <h3>{"遷移プロパティ"}</h3>
                        <div class="property-group">
                            <label>{"イベント:"}</label>
                            <input type="text" value={event} onchange={on_event_change} />
                        </div>
                        <div class="property-group">
                            <label>{"ソース:"}</label>
                            <input type="text" value={source} disabled={true} />
                        </div>
                        <div class="property-group">
                            <label>{"ターゲット:"}</label>
                            <input type="text" value={target} disabled={true} />
                        </div>
                    </div>
                };
            }
        }
        
        html! {
            <div class="no-selection">
                <p>{"要素を選択してください"}</p>
            </div>
        }
    };

    html! {
        <div class="properties-panel">
            {render_state_properties}
        </div>
    }
}