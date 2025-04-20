use crate::editor::EditorState;
use rustate::machine::StateMachine;
use web_sys::MouseEvent;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CanvasProps {
    pub editor_state: UseStateHandle<EditorState>,
}

#[function_component(Canvas)]
pub fn canvas(props: &CanvasProps) -> Html {
    let dragging = use_state(|| false);
    let selected_element = use_state(|| None::<String>);

    let on_canvas_click = {
        let selected_element = selected_element.clone();
        let editor_state = props.editor_state.clone();
        
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let target_id = e.target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| {
                    if el.has_attribute("data-state-id") {
                        el.get_attribute("data-state-id")
                    } else if el.has_attribute("data-transition-id") {
                        el.get_attribute("data-transition-id")
                    } else {
                        None
                    }
                });
            
            if let Some(id) = target_id {
                selected_element.set(Some(id));
                editor_state.set((*editor_state).with_selected_element(Some(id)));
            } else {
                selected_element.set(None);
                editor_state.set((*editor_state).with_selected_element(None));
            }
        })
    };

    let render_states = {
        let editor_state = &*props.editor_state;
        let machine = &editor_state.machine;
        
        machine.states.iter().map(|(id, state)| {
            let is_selected = editor_state.selected_element
                .as_ref()
                .map_or(false, |selected_id| selected_id == id);
            
            let state_class = if is_selected {
                "state state-selected"
            } else {
                "state"
            };
            
            let x = state.metadata.get("x").and_then(|v| v.as_f64()).unwrap_or(100.0) as i32;
            let y = state.metadata.get("y").and_then(|v| v.as_f64()).unwrap_or(100.0) as i32;
            
            html! {
                <div 
                    class={state_class}
                    style={format!("left: {}px; top: {}px;", x, y)}
                    data-state-id={id.clone()}
                >
                    <div class="state-name">{&state.name}</div>
                    {
                        if state.is_final {
                            html! { <div class="state-final-marker"></div> }
                        } else {
                            html! {}
                        }
                    }
                </div>
            }
        }).collect::<Html>()
    };

    let render_transitions = {
        let editor_state = &*props.editor_state;
        let machine = &editor_state.machine;
        
        machine.transitions.iter().map(|(id, transition)| {
            let is_selected = editor_state.selected_element
                .as_ref()
                .map_or(false, |selected_id| selected_id == id);
            
            let transition_class = if is_selected {
                "transition transition-selected"
            } else {
                "transition"
            };
            
            // 簡略化した直線の遷移表示
            let from_state = machine.states.get(&transition.source);
            let to_state = machine.states.get(&transition.target);
            
            if let (Some(from), Some(to)) = (from_state, to_state) {
                let from_x = from.metadata.get("x").and_then(|v| v.as_f64()).unwrap_or(100.0);
                let from_y = from.metadata.get("y").and_then(|v| v.as_f64()).unwrap_or(100.0);
                let to_x = to.metadata.get("x").and_then(|v| v.as_f64()).unwrap_or(200.0);
                let to_y = to.metadata.get("y").and_then(|v| v.as_f64()).unwrap_or(200.0);
                
                html! {
                    <svg class={transition_class} data-transition-id={id.clone()}>
                        <line 
                            x1={from_x.to_string()} 
                            y1={from_y.to_string()} 
                            x2={to_x.to_string()} 
                            y2={to_y.to_string()}
                            stroke="black"
                            stroke-width="2"
                        />
                        <text 
                            x={(from_x + to_x) / 2.0}
                            y={(from_y + to_y) / 2.0 - 10.0}
                            text-anchor="middle"
                        >
                            {transition.event.clone().unwrap_or_default()}
                        </text>
                    </svg>
                }
            } else {
                html! {}
            }
        }).collect::<Html>()
    };

    html! {
        <div class="canvas-container" onclick={on_canvas_click}>
            <div class="canvas">
                {render_states}
                {render_transitions}
            </div>
        </div>
    }
}