use super::{ProtoError, Result, xstate_machine};
use crate::{
    Action, ActionType, Guard, Machine, MachineBuilder, 
    State, StateType, Transition
};
use serde_json::Value;
use std::collections::HashMap;

/// Convert from Protocol Buffer representation to rustate's Machine
pub fn convert_from_proto(request: xstate_machine::ImportMachineRequest) -> Result<Machine> {
    let definition = request.definition.ok_or_else(|| {
        ProtoError::InvalidDefinition("Missing machine definition".to_string())
    })?;
    
    // Start building a machine
    let mut builder = MachineBuilder::new(&definition.id);
    
    // Set the initial state
    if !definition.initial.is_empty() {
        builder = builder.initial(&definition.initial);
    }
    
    // Convert states
    let states = convert_states(&definition.states)?;
    for state in states {
        builder = builder.state(state);
    }
    
    // Convert transitions from the top-level 'on' field
    if let Some(transitions) = convert_top_level_transitions(&definition.on, &definition.id)? {
        for transition in transitions {
            builder = builder.transition(transition);
        }
    }
    
    // Convert the context if available
    if definition.context.len() > 0 {
        // Parse context from the map to JSON
        let context = serde_json::to_value(&definition.context).map_err(|e| {
            ProtoError::ConversionError(format!("Failed to parse context: {}", e))
        })?;
        
        builder = builder.context(context);
    }
    
    // Convert global entry actions
    for entry_action in definition.entry {
        let action = convert_action(&entry_action)?;
        builder = builder.on_entry(&definition.id, action);
    }
    
    // Convert global exit actions
    for exit_action in definition.exit {
        let action = convert_action(&exit_action)?;
        builder = builder.on_exit(&definition.id, action);
    }
    
    // Build the machine
    builder.build().map_err(|e| {
        ProtoError::InvalidDefinition(format!("Failed to build machine: {}", e))
    })
}

/// Convert to Protocol Buffer representation from rustate's Machine
pub fn convert_to_proto(machine: &Machine) -> Result<xstate_machine::StateMachineConfig> {
    let mut config = xstate_machine::StateMachineConfig {
        id: machine.id().to_string(),
        version: "1.0".to_string(),
        type_: xstate_machine::StateType::Compound as i32,
        initial: machine.initial_state().to_string(),
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
    
    // Convert states
    for state in machine.states() {
        let proto_state = convert_state_to_proto(state, machine)?;
        config.states.insert(state.id().to_string(), proto_state);
    }
    
    // Convert context if available
    if let Some(ctx) = machine.context_raw() {
        if let Value::Object(map) = ctx {
            for (key, value) in map {
                let value_str = value.to_string();
                config.context.insert(key.clone(), value_str);
            }
        }
    }
    
    Ok(config)
}

/// Convert XState states to rustate states
fn convert_states(
    states_map: &HashMap<String, xstate_machine::StateNode>
) -> Result<Vec<State>> {
    let mut result = Vec::new();
    
    for (id, state_node) in states_map {
        let state_type = match xstate_machine::StateType::try_from(state_node.type_) {
            Ok(t) => t,
            Err(_) => return Err(ProtoError::InvalidDefinition(
                format!("Invalid state type for state {}", id)
            )),
        };
        
        let mut state = match state_type {
            xstate_machine::StateType::Final => State::new_final(id),
            xstate_machine::StateType::History => {
                let history_type = if state_node.history == "deep" {
                    crate::HistoryType::Deep
                } else {
                    crate::HistoryType::Shallow
                };
                State::new_history(id, history_type)
            },
            xstate_machine::StateType::Parallel => State::new_parallel(id),
            _ => State::new(id),
        };
        
        // Add tags if present
        for tag in &state_node.tags {
            state = state.tag(tag);
        }
        
        // Add metadata if present
        if !state_node.meta.is_empty() {
            let meta_json = serde_json::to_value(&state_node.meta).map_err(|e| {
                ProtoError::JsonError(e)
            })?;
            state = state.meta(meta_json);
        }
        
        result.push(state);
        
        // Recursively process child states
        if !state_node.states.is_empty() {
            let child_states = convert_states(&state_node.states)?;
            result.extend(child_states);
        }
    }
    
    Ok(result)
}

/// Convert XState transitions to rustate transitions
fn convert_top_level_transitions(
    transitions: &[xstate_machine::Transition],
    parent_id: &str,
) -> Result<Option<Vec<Transition>>> {
    if transitions.is_empty() {
        return Ok(None);
    }
    
    let mut result = Vec::new();
    
    for transition in transitions {
        // For each target, create a separate transition
        if transition.target.is_empty() {
            // Self-transition (no target)
            let mut t = Transition::new(parent_id, &transition.event, parent_id);
            
            // Add guards
            for guard in &transition.guards {
                let g = convert_guard(guard)?;
                t = t.guard(g);
            }
            
            result.push(t);
        } else {
            for target in &transition.target {
                let mut t = Transition::new(parent_id, &transition.event, target);
                
                // Add guards
                for guard in &transition.guards {
                    let g = convert_guard(guard)?;
                    t = t.guard(g);
                }
                
                result.push(t);
            }
        }
    }
    
    Ok(Some(result))
}

/// Convert XState guard to rustate guard
fn convert_guard(guard: &xstate_machine::Guard) -> Result<Guard> {
    Ok(Guard::new(&guard.name, move |ctx, evt| {
        // For now, use a simple implementation that always returns true
        // In a real implementation, this would evaluate the condition string
        true
    }))
}

/// Convert XState action to rustate action
fn convert_action(action: &xstate_machine::Action) -> Result<Action> {
    let action_type = match xstate_machine::ActionType::try_from(action.type_) {
        Ok(t) => match t {
            xstate_machine::ActionType::Entry => ActionType::Entry,
            xstate_machine::ActionType::Exit => ActionType::Exit,
            _ => ActionType::Transition,
        },
        Err(_) => return Err(ProtoError::InvalidDefinition(
            format!("Invalid action type for action {}", action.name)
        )),
    };
    
    Ok(Action::new(&action.name, action_type, move |ctx, evt| {
        // For now, use a simple implementation that does nothing
        // In a real implementation, this would execute the action based on its type
    }))
}

/// Convert rustate state to XState protocol buffer state
fn convert_state_to_proto(
    state: &State,
    machine: &Machine,
) -> Result<xstate_machine::StateNode> {
    let state_type = match state.state_type() {
        StateType::Normal => xstate_machine::StateType::Atomic,
        StateType::Final => xstate_machine::StateType::Final,
        StateType::Parallel => xstate_machine::StateType::Parallel,
        StateType::History(_) => xstate_machine::StateType::History,
    };
    
    let mut proto_state = xstate_machine::StateNode {
        id: state.id().to_string(),
        type_: state_type as i32,
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
    
    // Add initial state if this is a compound state
    if let Some(initial) = machine.initial_substate(state.id()) {
        proto_state.initial = initial.to_string();
    }
    
    // Add transitions
    for transition in machine.transitions_from(state.id()) {
        let proto_transition = xstate_machine::Transition {
            event: transition.event().to_string(),
            target: vec![transition.target().to_string()],
            source: transition.source().to_string(),
            guards: Vec::new(), // TODO: Add guards
            actions: Vec::new(), // TODO: Add actions
            internal: false,
            metadata: HashMap::new(),
        };
        
        proto_state.transitions.push(proto_transition);
    }
    
    // Add tags
    for tag in state.tags() {
        proto_state.tags.push(tag.to_string());
    }
    
    // Add metadata if available
    if let Some(meta) = state.meta() {
        if let Value::Object(map) = meta {
            for (key, value) in map {
                proto_state.meta.insert(key.clone(), value.to_string());
            }
        }
    }
    
    // Add child states
    for child_id in machine.child_states(state.id()) {
        if let Some(child_state) = machine.get_state(child_id) {
            let proto_child = convert_state_to_proto(child_state, machine)?;
            proto_state.states.insert(child_id.to_string(), proto_child);
        }
    }
    
    Ok(proto_state)
} 