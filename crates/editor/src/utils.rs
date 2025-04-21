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
#[allow(dead_code)]
pub fn generate_rust_code(machine: &rustate::machine::Machine) -> String {
    let mut code = String::new();

    code.push_str("use rustate::prelude::*;\n\n");
    code.push_str(&format!(
        "fn create_state_machine() -> rustate::Machine {{\n"
    ));
    code.push_str(&format!(
        "    let mut builder = rustate::MachineBuilder::new(\"{}\");\n\n",
        machine.name
    ));

    // ステートの生成
    for (id, state) in &machine.states {
        match state.state_type {
            rustate::state::StateType::Normal => {
                code.push_str(&format!("    // State: {}\n", id));
                code.push_str(&format!(
                    "    builder = builder.state(rustate::State::new(\"{}\"));\n",
                    id
                ));
            }
            rustate::state::StateType::Final => {
                code.push_str(&format!("    // State: {}\n", id));
                code.push_str(&format!(
                    "    builder = builder.state(rustate::State::new_final(\"{}\"));\n",
                    id
                ));
            }
            rustate::state::StateType::Compound => {
                if let Some(initial) = &state.initial {
                    code.push_str(&format!("    // State: {}\n", id));
                    code.push_str(&format!(
                        "    builder = builder.state(rustate::State::new_compound(\"{}\", \"{}\"));\n",
                        id, initial
                    ));
                } else {
                    code.push_str(&format!(
                        "    // Warning: Compound state without initial state\n"
                    ));
                    code.push_str(&format!(
                        "    builder = builder.state(rustate::State::new(\"{}\"));\n",
                        id
                    ));
                }
            }
            rustate::state::StateType::Parallel => {
                code.push_str(&format!("    // State: {}\n", id));
                code.push_str(&format!(
                    "    builder = builder.state(rustate::State::new_parallel(\"{}\"));\n",
                    id
                ));
            }
            rustate::state::StateType::History => {
                code.push_str(&format!("    // State: {}\n", id));
                code.push_str(&format!(
                    "    builder = builder.state(rustate::State::new_history(\"{}\"));\n",
                    id
                ));
            }
            rustate::state::StateType::DeepHistory => {
                code.push_str(&format!("    // State: {}\n", id));
                code.push_str(&format!(
                    "    builder = builder.state(rustate::State::new_deep_history(\"{}\"));\n",
                    id
                ));
            }
        }
    }

    code.push_str("\n");

    // Set initial state
    if !machine.initial.is_empty() {
        code.push_str(&format!(
            "    builder = builder.initial(\"{}\");\n\n",
            machine.initial
        ));
    }

    // 遷移の追加
    for transition in &machine.transitions {
        if let Some(target) = &transition.target {
            code.push_str(&format!(
                "    builder = builder.transition(rustate::Transition::new(\"{}\", \"{}\", \"{}\"));\n",
                transition.source,
                transition.event,
                target
            ));
        } else {
            code.push_str(&format!(
                "    builder = builder.transition(rustate::Transition::internal_transition(\"{}\", \"{}\"));\n",
                transition.source,
                transition.event
            ));
        }
    }

    code.push_str("\n    rustate::Machine::new(builder).expect(\"Failed to build machine\")\n");
    code.push_str("}\n");

    code
}
