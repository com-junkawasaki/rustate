use rustate::proto::{
    xstate_machine::{self, StateType, ActionType, StateMachineConfig, ImportMachineRequest},
    import_machine_from_proto,
};
use std::collections::HashMap;
use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a traffic light state machine using the XState-compatible Protocol Buffer format
    let mut traffic_light = StateMachineConfig {
        id: "trafficLight".to_string(),
        version: "1.0".to_string(),
        type_: StateType::Compound as i32,
        initial: "green".to_string(),
        states: HashMap::new(),
        context: HashMap::new(),
        entry: Vec::new(),
        exit: Vec::new(),
        on: Vec::new(),
        meta: HashMap::new(),
        description: "A simple traffic light state machine".to_string(),
        tags: vec!["demo".to_string()],
        strict: Vec::new(),
        predictable_action_arguments: true,
    };
    
    // Create the green state
    let green = xstate_machine::StateNode {
        id: "green".to_string(),
        type_: StateType::Atomic as i32,
        initial: "".to_string(),
        states: HashMap::new(),
        transitions: vec![
            xstate_machine::Transition {
                event: "TIMER".to_string(),
                target: vec!["yellow".to_string()],
                source: "green".to_string(),
                guards: Vec::new(),
                actions: Vec::new(),
                internal: false,
                metadata: HashMap::new(),
            },
        ],
        after: HashMap::new(),
        entry: vec![
            xstate_machine::Action {
                name: "logGreen".to_string(),
                type_: ActionType::Entry as i32,
                action: "console.log('Entering GREEN state - Go!')".to_string(),
                params: HashMap::new(),
                assignments: HashMap::new(),
                metadata: HashMap::new(),
            },
        ],
        exit: Vec::new(),
        invoke: Vec::new(),
        meta: HashMap::new(),
        tags: Vec::new(),
        data: HashMap::new(),
        always: false,
        history: "".to_string(),
        context: HashMap::new(),
    };
    
    // Create the yellow state
    let yellow = xstate_machine::StateNode {
        id: "yellow".to_string(),
        type_: StateType::Atomic as i32,
        initial: "".to_string(),
        states: HashMap::new(),
        transitions: vec![
            xstate_machine::Transition {
                event: "TIMER".to_string(),
                target: vec!["red".to_string()],
                source: "yellow".to_string(),
                guards: Vec::new(),
                actions: Vec::new(),
                internal: false,
                metadata: HashMap::new(),
            },
        ],
        after: HashMap::new(),
        entry: vec![
            xstate_machine::Action {
                name: "logYellow".to_string(),
                type_: ActionType::Entry as i32,
                action: "console.log('Entering YELLOW state - Prepare to stop!')".to_string(),
                params: HashMap::new(),
                assignments: HashMap::new(),
                metadata: HashMap::new(),
            },
        ],
        exit: Vec::new(),
        invoke: Vec::new(),
        meta: HashMap::new(),
        tags: Vec::new(),
        data: HashMap::new(),
        always: false,
        history: "".to_string(),
        context: HashMap::new(),
    };
    
    // Create the red state
    let red = xstate_machine::StateNode {
        id: "red".to_string(),
        type_: StateType::Atomic as i32,
        initial: "".to_string(),
        states: HashMap::new(),
        transitions: vec![
            xstate_machine::Transition {
                event: "TIMER".to_string(),
                target: vec!["green".to_string()],
                source: "red".to_string(),
                guards: Vec::new(),
                actions: Vec::new(),
                internal: false,
                metadata: HashMap::new(),
            },
        ],
        after: HashMap::new(),
        entry: vec![
            xstate_machine::Action {
                name: "logRed".to_string(),
                type_: ActionType::Entry as i32,
                action: "console.log('Entering RED state - Stop!')".to_string(),
                params: HashMap::new(),
                assignments: HashMap::new(),
                metadata: HashMap::new(),
            },
        ],
        exit: Vec::new(),
        invoke: Vec::new(),
        meta: HashMap::new(),
        tags: Vec::new(),
        data: HashMap::new(),
        always: false,
        history: "".to_string(),
        context: HashMap::new(),
    };
    
    // Add the states to the state machine
    traffic_light.states.insert("green".to_string(), green);
    traffic_light.states.insert("yellow".to_string(), yellow);
    traffic_light.states.insert("red".to_string(), red);
    
    // Create a context (extended state)
    let mut context_map = HashMap::new();
    context_map.insert("cycles".to_string(), "0".to_string());
    context_map.insert("lastChange".to_string(), "0".to_string());
    traffic_light.context = context_map;
    
    // Create the import request
    let request = ImportMachineRequest {
        definition: Some(traffic_light),
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
    println!("Sending TIMER event...");
    machine.send("TIMER")?;
    println!("Current state: {}", machine.current_state());
    
    println!("Sending TIMER event...");
    machine.send("TIMER")?;
    println!("Current state: {}", machine.current_state());
    
    println!("Sending TIMER event...");
    machine.send("TIMER")?;
    println!("Current state: {}", machine.current_state());
    
    Ok(())
} 