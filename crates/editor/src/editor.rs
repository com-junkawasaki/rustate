use rustate::machine::{Machine, MachineBuilder};
use rustate::{Event, State};
use wasm_bindgen::prelude::*;

// エディタのメインクラス
#[wasm_bindgen]
pub struct Editor {
    state: EditorState,
}

#[wasm_bindgen]
impl Editor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            state: EditorState::new(),
        }
    }

    pub fn render(&self) -> Result<(), JsValue> {
        crate::init_editor("editor-container")
    }

    pub fn get_json(&self) -> String {
        serde_json::to_string_pretty(&self.state.machine).unwrap_or_default()
    }

    pub fn load_from_json(&mut self, json: &str) -> Result<(), JsValue> {
        match serde_json::from_str::<Machine>(json) {
            Ok(machine) => {
                self.state.machine = machine;
                Ok(())
            }
            Err(err) => {
                Err(JsValue::from_str(&format!("JSON解析エラー: {}", err)))
            }
        }
    }
}

// エディタの状態
#[derive(Clone)]
pub struct EditorState {
    pub machine: Machine,
    pub selected_element: Option<String>,
    pub mode: EditorMode,
}

// Manual implementation of PartialEq for EditorState since Machine doesn't implement it
impl PartialEq for EditorState {
    fn eq(&self, other: &Self) -> bool {
        // We can't compare machines directly, so we'll compare their serialized forms
        let this_machine = serde_json::to_string(&self.machine).unwrap_or_default();
        let other_machine = serde_json::to_string(&other.machine).unwrap_or_default();
        
        this_machine == other_machine
            && self.selected_element == other.selected_element
            && self.mode == other.mode
    }
}

impl EditorState {
    pub fn new() -> Self {
        // Create a default machine with builder with explicit type annotations
        let builder: MachineBuilder<State, Event> = MachineBuilder::new("default_machine")
            .initial("initial")
            .state(rustate::State::new("initial"));
            
        // Build the machine
        let machine = Machine::new(builder).expect("Failed to create default machine");

        Self {
            machine,
            selected_element: None,
            mode: EditorMode::Select,
        }
    }

    pub fn with_selected_element(mut self, element_id: Option<String>) -> Self {
        self.selected_element = element_id;
        self
    }
}

// エディタの操作モード
#[derive(Clone, PartialEq)]
pub enum EditorMode {
    Select,
    AddState,
    AddTransition,
}