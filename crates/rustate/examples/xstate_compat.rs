use rustate::proto::{
    xstate_machine::{self, ImportMachineRequest, StateMachineConfig},
    import_machine_from_proto,
};
use prost::Message;
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This is a JSON definition of a state machine in XState format
    let xstate_json = r#"
    {
      "id": "toggle",
      "initial": "inactive",
      "states": {
        "inactive": {
          "on": {
            "TOGGLE": "active"
          }
        },
        "active": {
          "on": {
            "TOGGLE": "inactive"
          }
        }
      }
    }
    "#;
    
    // Parse the XState JSON
    let xstate_value: serde_json::Value = serde_json::from_str(xstate_json)?;
    
    // Convert the XState JSON to a Protocol Buffer definition
    let mut machine_config = StateMachineConfig {
        id: xstate_value["id"].as_str().unwrap_or("machine").to_string(),
        version: "1.0".to_string(),
        type_: xstate_machine::StateType::Compound as i32,
        initial: xstate_value["initial"].as_str().unwrap_or("").to_string(),
        states: HashMap::new(),
        context: HashMap::new(),
        entry: Vec::new(),
        exit: Vec::new(),
        on: Vec::new(),
        meta: HashMap::new(),
        description: "".to_string(),
        tags: Vec::new(),
        strict: Vec::new(),
        predictable_action_arguments: true,
    };
    
    // Convert XState states to Protocol Buffer states
    if let Some(states) = xstate_value["states"].as_object() {
        for (state_id, state_def) in states {
            let mut state_node = xstate_machine::StateNode {
                id: state_id.clone(),
                type_: xstate_machine::StateType::Atomic as i32,
                initial: "".to_string(),
                states: HashMap::new(),
                transitions: Vec::new(),
                after: HashMap::new(),
                entry: Vec::new(),
                exit: Vec::new(),
                invoke: Vec::new(),
                meta: HashMap::new(),
                tags: Vec::new(),
                data: HashMap::new(),
                always: false,
                history: "".to_string(),
                context: HashMap::new(),
            };
            
            // Convert transitions
            if let Some(on) = state_def["on"].as_object() {
                for (event, target) in on {
                    let target_str = if target.is_string() {
                        target.as_str().unwrap_or("").to_string()
                    } else if target.is_object() && target["target"].is_string() {
                        target["target"].as_str().unwrap_or("").to_string()
                    } else {
                        continue;
                    };
                    
                    let transition = xstate_machine::Transition {
                        event: event.clone(),
                        target: vec![target_str],
                        source: state_id.clone(),
                        guards: Vec::new(),
                        actions: Vec::new(),
                        internal: false,
                        metadata: HashMap::new(),
                    };
                    
                    state_node.transitions.push(transition);
                }
            }
            
            machine_config.states.insert(state_id.clone(), state_node);
        }
    }
    
    // Create the import request
    let request = ImportMachineRequest {
        definition: Some(machine_config),
        options: None,
    };
    
    // Encode the request to bytes
    let mut bytes = Vec::new();
    request.encode(&mut bytes)?;
    
    // Import the machine from Protocol Buffer format
    let machine = import_machine_from_proto(&bytes)?;
    
    // Use the machine
    println!("Initial state: {}", machine.current_state());
    
    // Send events to the machine
    println!("Sending TOGGLE event...");
    machine.send("TOGGLE")?;
    println!("Current state: {}", machine.current_state());
    
    println!("Sending TOGGLE event...");
    machine.send("TOGGLE")?;
    println!("Current state: {}", machine.current_state());
    
    // Export the machine back to JSON (XState compatible)
    let machine_json = json!({
        "id": machine.id(),
        "initial": machine.initial_state(),
        "states": {
            "inactive": {
                "on": {
                    "TOGGLE": "active"
                }
            },
            "active": {
                "on": {
                    "TOGGLE": "inactive"
                }
            }
        }
    });
    
    println!("\nExported XState JSON:\n{}", 
        serde_json::to_string_pretty(&machine_json)?);
    
    Ok(())
} 