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
pub fn generate_rust_code(machine: &rustate::machine::StateMachine) -> String {
    let mut code = String::new();
    
    code.push_str("use rustate::prelude::*;\n\n");
    code.push_str("fn create_state_machine() -> StateMachine {\n");
    code.push_str("    let mut machine = StateMachine::new();\n\n");
    
    // ステートの生成
    for (id, state) in &machine.states {
        code.push_str(&format!(
            "    let {} = State::new(\"{}\"){};\n",
            id.replace("-", "_"),
            state.name,
            if state.is_final { ".final_state()" } else { "" }
        ));
    }
    
    code.push_str("\n");
    
    // マシンにステートを追加
    for (id, _) in &machine.states {
        code.push_str(&format!(
            "    machine.add_state({});\n",
            id.replace("-", "_")
        ));
    }
    
    code.push_str("\n");
    
    // 遷移の追加
    for (_, transition) in &machine.transitions {
        let event = transition.event.as_ref().unwrap_or(&String::from(""));
        code.push_str(&format!(
            "    machine.add_transition(\"{}\", {}, {});\n",
            event,
            transition.source.replace("-", "_"),
            transition.target.replace("-", "_")
        ));
    }
    
    code.push_str("\n    machine\n");
    code.push_str("}\n");
    
    code
}