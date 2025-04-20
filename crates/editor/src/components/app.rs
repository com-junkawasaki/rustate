use crate::components::{Canvas, EditorPanel, PropertiesPanel, Toolbar};
use crate::editor::EditorState;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    let editor_state = use_state(EditorState::new);

    html! {
        <div class="editor-app">
            <Toolbar editor_state={editor_state.clone()} />
            <div class="main-content">
                <EditorPanel editor_state={editor_state.clone()} />
                <Canvas editor_state={editor_state.clone()} />
                <PropertiesPanel editor_state={editor_state.clone()} />
            </div>
        </div>
    }
}