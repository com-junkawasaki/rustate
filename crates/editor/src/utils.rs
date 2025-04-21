use wasm_bindgen::prelude::*;

// コンソールログ用のヘルパー関数
#[wasm_bindgen]
pub fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(s));
}

// エラーログ用のヘルパー関数
#[wasm_bindgen]
pub fn error(s: &str) {
    web_sys::console::error_1(&JsValue::from_str(s));
}

// ステートマシンをRustコードに変換するヘルパー関数
pub fn generate_rust_code(machine: &rustate::machine::Machine) -> String {
    let mut code = String::new();
    
    code.push_str("use rustate::prelude::*;\n\n");
    code.push_str(&format!("fn create_state_machine() -> rustate::Machine {{\n"));
    code.push_str(&format!("    let mut builder = rustate::MachineBuilder::new(\"{}\");\n\n", 
        machine.name));
    
    // ステートの生成
    for (id, state) in &machine.states {
        let state_type = match state.state_type {
            rustate::state::StateType::Normal => "Normal",
            rustate::state::StateType::Initial => "Initial",
            rustate::state::StateType::Final => "Final",
            rustate::state::StateType::Compound => "Compound",
            rustate::state::StateType::Parallel => "Parallel",
            rustate::state::StateType::History => "History",
            _ => "Normal", // Default
        };
        
        code.push_str(&format!(
            "    builder = builder.state(rustate::State::new(\"{}\", rustate::state::StateType::{}){};\n",
            id,
            state_type,
            if id == &machine.initial { ".make_initial()" } else { "" }
        ));
    }
    
    code.push_str("\n");
    
    // 遷移の追加
    for transition in &machine.transitions {
        if let Some(target) = &transition.target {
            let event = transition.event.as_ref()
                .map(|e| format!("Some(\"{}\")", e))
                .unwrap_or_else(|| "None".to_string());
                
            code.push_str(&format!(
                "    builder = builder.transition(rustate::Transition::new(\"{}\", \"{}\", {}));\n",
                transition.source,
                target,
                event
            ));
        }
    }
    
    code.push_str("\n    rustate::Machine::new(builder).expect(\"Failed to build machine\")\n");
    code.push_str("}\n");
    
    code
}