use crate::editor::EditorState;
use yew::prelude::*;
use wasm_bindgen::JsCast;
use rustate::state::StateType;

#[derive(Properties, PartialEq)]
pub struct PropertiesPanelProps {
    pub editor_state: UseStateHandle<EditorState>,
}

#[function_component(PropertiesPanel)]
pub fn properties_panel(props: &PropertiesPanelProps) -> Html {
    let render_state_properties = {
        let editor_state = props.editor_state.clone();
        
        if let Some(element_id) = &editor_state.selected_element {
            if let Some(state) = editor_state.machine.states.get(element_id) {
                let name = state.id.clone();
                let is_final = state.state_type == StateType::Final;
                
                let _on_name_change = {
                    let editor_state = editor_state.clone();
                    let element_id = element_id.clone();
                    
                    Callback::from(move |e: Event| {
                        let target = e.target().unwrap();
                        let _input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
                        let _new_state = (*editor_state).clone();
                        
                        if let Some(_state) = _new_state.machine.states.get(&element_id).cloned() {
                            web_sys::console::warn_1(&"ステートIDの変更はサポートされていません".into());
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
                        
                        if let Some(old_state) = new_state.machine.states.get(&element_id).cloned() {
                            let new_state_obj = if input.checked() {
                                rustate::State::new_final(&old_state.id)
                            } else {
                                rustate::State::new(&old_state.id)
                            };
                            
                            new_state.machine.states.insert(element_id.clone(), new_state_obj);
                            editor_state.set(new_state);
                        }
                    })
                };
                
                return html! {
                    <div class="state-properties">
                        <h3>{"ステートプロパティ"}</h3>
                        <div class="property-group">
                            <label>{"ID:"}</label>
                            <input type="text" value={name} disabled={true} title="IDは変更できません" />
                        </div>
                        <div class="property-group">
                            <label>{"終了ステート:"}</label>
                            <input type="checkbox" checked={is_final} onchange={on_final_change} />
                        </div>
                        <div class="property-group">
                            <label>{"ステートタイプ:"}</label>
                            <select disabled={true}>
                                <option selected={state.state_type == StateType::Normal}>{"Normal"}</option>
                                <option selected={state.state_type == StateType::Final}>{"Final"}</option>
                                <option selected={state.state_type == StateType::Compound}>{"Compound"}</option>
                                <option selected={state.state_type == StateType::Parallel}>{"Parallel"}</option>
                                <option selected={state.state_type == StateType::History}>{"History"}</option>
                                <option selected={state.state_type == StateType::DeepHistory}>{"DeepHistory"}</option>
                            </select>
                        </div>
                    </div>
                };
            } else if element_id.starts_with("transition-") {
                if let Some(index_str) = element_id.strip_prefix("transition-") {
                    if let Ok(index) = index_str.parse::<usize>() {
                        if index < editor_state.machine.transitions.len() {
                            let transition = &editor_state.machine.transitions[index];
                            let event = transition.event.clone();
                            let source = transition.source.clone();
                            let target = transition.target.clone().unwrap_or_default();
                            
                            let on_event_change = {
                                let editor_state = editor_state.clone();
                                let element_id = element_id.clone();
                                
                                Callback::from(move |e: Event| {
                                    let target_el = e.target().unwrap();
                                    let input = target_el.dyn_into::<web_sys::HtmlInputElement>().unwrap();
                                    let mut new_state = (*editor_state).clone();
                                    
                                    if let Some(index_str) = element_id.strip_prefix("transition-") {
                                        if let Ok(index) = index_str.parse::<usize>() {
                                            if index < new_state.machine.transitions.len() {
                                                let mut new_transitions = new_state.machine.transitions.clone();
                                                let mut transition = new_transitions[index].clone();
                                                
                                                let updated_transition = rustate::Transition::new(
                                                    &transition.source,
                                                    input.value(),
                                                    transition.target.as_ref().unwrap_or(&String::new())
                                                );
                                                
                                                new_transitions[index] = updated_transition;
                                                new_state.machine.transitions = new_transitions;
                                                editor_state.set(new_state);
                                            }
                                        }
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
                }
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