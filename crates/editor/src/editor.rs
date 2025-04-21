use rustate::machine::Machine;
use serde::{Deserialize, Serialize};
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
#[derive(Clone, PartialEq)]
pub struct EditorState {
    pub machine: Machine,
    pub selected_element: Option<String>,
    pub mode: EditorMode,
}

impl EditorState {
    pub fn new() -> Self {
        // Create a default machine
        let machine = Machine::new(
            rustate::MachineBuilder::new("default_machine")
                .initial("initial")
                .state(rustate::State::new("initial", rustate::state::StateType::Normal))
                .build()
                .unwrap_or_else(|_| panic!("Failed to create default machine"))
        );

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