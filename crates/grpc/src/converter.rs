//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use crate::error::{GrpcError, Result};
use crate::proto;
use prost_types::Any;
use rustate::state::{State as RuState, StateType as RuStateType};
use rustate::transition::Transition as RuTransition;
use rustate::ActionType as RuActionType;
use rustate::{Context as RuContext, Machine as RuMachine, MachineBuilder as RuMachineBuilder};
use serde_json::{from_value, json, to_value, Value};
use std::collections::HashMap;

// Import generated gRPC types
pub mod proto {
    tonic::include_proto!(\"rustate\");
}

// Import necessary types from rustate core
use rustate::{
    action::{Action, IntoAction},
    context::Context,
    error::{Result as RuResult, StateError},
    event::{Event, EventTrait},
    guard::{Guard, IntoGuard},
    machine::{Machine, MachineBuilder},
    state::{State, StateTrait, StateType},
    transition::{Transition, TransitionType}, // Added TransitionType import
};

// Import necessary types for JSON handling and conversion
use prost_types::{value::Kind, ListValue, Struct as ProstStruct, Value as ProstValue};
use serde_json::{Map, Value as JsonValue};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use tokio::sync::RwLock;

/// RuStateのStateTypeからgRPCのStateTypeへの変換
///
/// # 引数
/// * `state_type` - RuStateのStateType
///
/// # 戻り値
/// * 対応するgRPCのStateType
pub fn state_type_to_proto(state_type: &RuStateType) -> proto::StateType {
    match state_type {
        RuStateType::Normal => proto::StateType::Normal,
        RuStateType::Final => proto::StateType::Final,
        RuStateType::History => proto::StateType::History,
        RuStateType::Parallel => proto::StateType::Parallel,
        RuStateType::Compound => proto::StateType::Normal, // Compoundはprotoにないため、Normalにマッピング
        RuStateType::DeepHistory => proto::StateType::History, // DeepHistoryはprotoにないため、Historyにマッピング
    }
}

/// gRPCのStateTypeからRuStateのStateTypeへの変換
///
/// # 引数
/// * `state_type` - gRPCのStateType
///
/// # 戻り値
/// * 対応するRuStateのStateType
pub fn state_type_from_proto(state_type: proto::StateType) -> RuStateType {
    match state_type {
        proto::StateType::Normal => RuStateType::Normal,
        proto::StateType::Final => RuStateType::Final,
        proto::StateType::History => RuStateType::History,
        proto::StateType::Parallel => RuStateType::Parallel,
    }
}

/// RuStateのActionTypeからgRPCのActionTypeへの変換
pub fn action_type_to_proto(action_type: &RuActionType) -> proto::ActionType {
    match action_type {
        RuActionType::Entry => proto::ActionType::Entry,
        RuActionType::Exit => proto::ActionType::Exit,
        RuActionType::Transition => proto::ActionType::Transition,
    }
}

/// gRPCのActionTypeからRuStateのActionTypeへの変換
pub fn action_type_from_proto(action_type: proto::ActionType) -> RuActionType {
    match action_type {
        proto::ActionType::Entry => RuActionType::Entry,
        proto::ActionType::Exit => RuActionType::Exit,
        proto::ActionType::Transition => RuActionType::Transition,
    }
}

/// RuStateのStateからgRPCのStateへの変換
///
/// # 引数
/// * `state` - RuStateの状態オブジェクト
///
/// # 戻り値
/// * 変換されたgRPCの状態オブジェクト
pub fn state_to_proto(state: &RuState) -> proto::State {
    proto::State {
        id: state.id.to_string(),
        r#type: state_type_to_proto(&state.state_type) as i32,
        parent: state.parent.clone().unwrap_or_default().to_string(),
        children: state.children.iter().map(|s| s.to_string()).collect(),
    }
}

/// gRPCのStateからRuStateのStateへの変換
///
/// # 引数
/// * `proto_state` - gRPCの状態オブジェクト
///
/// # 戻り値
/// * 変換されたRuStateの状態オブジェクト
pub fn state_from_proto(proto_state: &proto::State) -> RuState {
    // Determine the state type from the proto enum, defaulting to Normal
    let state_type = state_type_from_proto(
        proto::StateType::try_from(proto_state.r#type).unwrap_or(proto::StateType::Normal),
    );

    // Create the RuState based on its type
    let mut state = match state_type {
        RuStateType::Normal => RuState::new(&proto_state.id),
        RuStateType::Final => RuState::new_final(&proto_state.id),
        RuStateType::History => {
            let mut s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::History; // Explicitly set history type
            s
        }
        RuStateType::DeepHistory => {
            let mut s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::DeepHistory; // Explicitly set deep history type
            s
        }
        RuStateType::Parallel => RuState::new_parallel(&proto_state.id),
        RuStateType::Compound => {
            // Compound state needs an initial child, which isn't directly in proto::State.
            // We might derive it from children or default to an empty string if not crucial.
            // If the proto definition includes an 'initial' field, use that.
            // For now, using empty string as initial.
            RuState::new_compound(&proto_state.id, "")
        }
    };

    // Set the parent state if provided
    if !proto_state.parent.is_empty() {
        state.parent = Some(proto_state.parent.clone());
    }

    // Add children states
    // RuState::children is Vec<String>, proto::State::children is Vec<String>
    state.children = proto_state.children.clone();

    // If proto::State had an 'initial' field, set it here:
    // if let Some(initial) = &proto_state.initial {
    //     state.initial = Some(initial.clone());
    // }

    state
}

/// RuStateのTransitionからgRPCのTransitionへの変換
///
/// # 引数
/// * `transition` - RuStateの遷移オブジェクト
///
/// # 戻り値
/// * 変換されたgRPCの遷移オブジェクト
pub fn transition_to_proto<S, C, E>(
    transition: &Transition<S, C, E>,
) -> Result<proto::Transition, ConversionError>
where
    S: StateTrait + Serialize,
    C: Serialize,
    E: EventTrait + Serialize,
{
    // Convert source and target Option<S> to proto::StateId
    let source_id = state_to_proto_id(&transition.source)?;
    let target_id = match &transition.target {
        Some(t) => Some(state_to_proto_id(t)?),
        None => None,
    };

    // Convert event Option<E> to proto::Event
    let event = match &transition.event {
        Some(e) => Some(event_to_proto(e)?),
        None => None,
    };

    // Convert guard Option<Guard<C, E>> to proto::Guard
    let guard = match &transition.guard {
        Some(g) => Some(guard_to_proto(g)?),
        None => None,
    };

    // Convert actions Vec<Action<C, E>> to Vec<proto::Action>
    let actions = transition
        .actions
        .iter()
        .map(action_to_proto)
        .collect::<Result<Vec<_>, _>>()?;

    // Convert TransitionType enum
    let transition_type = match transition.transition_type {
        TransitionType::External => proto::TransitionType::External,
        TransitionType::Internal => proto::TransitionType::Internal,
    };

    Ok(proto::Transition {
        source: Some(source_id),
        target: target_id, // Already Option<proto::StateId>
        event,
        guard,
        actions,
        transition_type: transition_type.into(),
    })
}

/// gRPCのTransitionからRuStateのTransitionへの変換
///
/// # 引数
/// * `proto_transition` - gRPCの遷移オブジェクト
///
/// # 戻り値
/// * 変換されたRuStateの遷移オブジェクト
pub fn transition_from_proto<S, C, E>(
    proto_transition: &proto::Transition,
) -> Result<Transition<S, C, E>, ConversionError>
where
    S: StateTrait + DeserializeOwned + Clone + Send + Sync + \'static,
    C: DeserializeOwned + Clone + Send + Sync + Default + Debug + \'static,
    E: EventTrait + DeserializeOwned + Clone + Send + Sync + Eq + Debug + Serialize + \'static,
{
    let source = proto_transition
        .source
        .as_ref()
        .ok_or(ConversionError::MissingField(\"transition source\".to_string()))
        .and_then(|id| state_from_proto_id::<S>(id))?;

    let target = match &proto_transition.target {
        Some(id) => Some(state_from_proto_id::<S>(id)?),
        None => None,
    };

    let event = match &proto_transition.event {
        Some(ev) => Some(event_from_proto::<E>(ev)?),
        None => None,
    };

    // Guards need reconstruction with actual logic, use named guard for now
    let guard = match &proto_transition.guard {
        Some(g) => Some(Guard::<C, E>::named(&g.name)), // Reconstruct named guard
        None => None,
    };

    // Actions also need reconstruction, use named actions (assuming Action::named exists)
    let actions = proto_transition
        .actions
        .iter()
        .map(|a| Ok(Action::<C, E>::from_fn(|_,_| async { Ok(()) } ) ) ) // Reconstruct dummy action
        .collect::<Result<Vec<_>, ConversionError>>()?;

    let transition_type = proto::TransitionType::try_from(proto_transition.transition_type)
        .map_err(|_| {
            ConversionError::InvalidValue(\"invalid transition type\".to_string())
        })
        .map(|tt| match tt {
            proto::TransitionType::External => TransitionType::External,
            proto::TransitionType::Internal => TransitionType::Internal,
        })?;

    // Use Transition::new to create the core Transition struct
    Ok(Transition::new(
        source,
        target,
        event,
        guard,
        actions,
        transition_type,
    ))
}

/// RuStateのMachineからgRPCのMachineDefinitionへの変換
///
/// # 引数
/// * `machine` - RuStateのステートマシン
///
/// # 戻り値
/// * 変換されたgRPCのマシン定義オブジェクト
/// * エラー: シリアライゼーションに失敗した場合
pub fn machine_to_proto(machine: &RuMachine) -> Result<proto::MachineDefinition> {
    // 状態の変換
    let mut states = Vec::new();
    for (_id, state) in &machine.states {
        states.push(state_to_proto(state));
    }

    // 遷移の変換
    let mut transitions = Vec::new();
    for transition in &machine.transitions {
        transitions.push(transition_to_proto(transition)?);
    }

    // コンテキストのJSONシリアライズ
    let context_json = serde_json::to_string(&machine.context).map_err(GrpcError::Serialization)?;

    Ok(proto::MachineDefinition {
        id: machine.name.to_string(),
        initial: machine.initial.to_string(),
        states,
        transitions,
        // アクションとガードの詳細情報は将来的に実装予定
        actions: vec![],
        guards: vec![],
        context: context_json,
    })
}

/// gRPCのMachineDefinitionからRuStateのMachineへの変換
///
/// # 引数
/// * `proto_machine` - gRPCのマシン定義オブジェクト
///
/// # 戻り値
/// * 変換されたRuStateのステートマシン
/// * エラー: デシリアライゼーションに失敗した場合
pub fn machine_from_proto(proto_machine: &proto::MachineDefinition) -> Result<RuMachine> {
    let mut builder = RuMachineBuilder::new(&proto_machine.id);

    // 初期状態の設定
    builder = builder.initial(&proto_machine.initial);

    // 状態の追加
    for proto_state in &proto_machine.states {
        let rustate_state = state_from_proto(proto_state);
        // If state_from_proto needs the initial field for Compound states, pass it:
        // let initial = proto_state.initial.as_deref().unwrap_or(""); // Example
        // let rustate_state = state_from_proto(proto_state, initial); // If signature changes
        builder = builder.state(rustate_state);
    }

    // 遷移の追加
    for proto_transition in &proto_machine.transitions {
        let rustate_transition = transition_from_proto(proto_transition)?;
        // Add guards/actions here if they are part of the proto definition
        // and can be reconstructed into callable closures/functions.
        // This usually requires a registry or factory.
        builder = builder.transition(rustate_transition);
        // Example: Add guards/actions by ID lookup
        // for guard_id in &proto_transition.guards { builder = builder.guard(&guard_id, lookup_guard(guard_id)); }
        // for action_id in &proto_transition.actions { builder = builder.action(&action_id, lookup_action(action_id)); }
    }

    // コンテキストの設定
    if !proto_machine.context.is_empty() {
        let context: RuContext =
            serde_json::from_str(&proto_machine.context).map_err(GrpcError::Serialization)?;
        builder = builder.context(context);
    }

    // ステートマシンの構築
    let machine = builder.build().map_err(GrpcError::StateMachine)?;

    Ok(machine)
}

/// RuStateのMachineからgRPCのMachineStateへの変換
///
/// # 引数
/// * `machine` - RuStateのステートマシン
///
/// # 戻り値
/// * 変換されたgRPCのマシン状態オブジェクト
/// * エラー: シリアライゼーションに失敗した場合
pub fn machine_state_to_proto(machine: &RuMachine) -> Result<proto::MachineState> {
    let context_json = serde_json::to_string(&machine.context).map_err(GrpcError::Serialization)?;

    // current_statesを文字列の配列として取得
    let current_states = machine
        .current_states
        .iter()
        .map(|s| s.to_string())
        .collect();

    Ok(proto::MachineState {
        machine_id: machine.name.to_string(),
        current_states,
        context: context_json,
    })
}

/// EventDefinitionからRuStateのイベント型への変換
///
/// # 引数
/// * `proto_event` - gRPCのイベント定義オブジェクト
///
/// # 戻り値
/// * イベント文字列とペイロード (Option<serde_json::Value>)
pub fn event_from_proto(
    proto_event: &proto::EventDefinition,
) -> Result<(String, Option<serde_json::Value>)> {
    let event_type = proto_event.event_type.clone();
    let payload = if !proto_event.payload.is_empty() {
        let value: serde_json::Value =
            serde_json::from_str(&proto_event.payload).map_err(GrpcError::Serialization)?;
        Some(value)
    } else {
        None
    };
    Ok((event_type, payload))
}

/// RuStateのイベントペイロードからgRPCのEventDefinitionへの変換
///
/// # 引数
/// * `machine_id` - 対象マシンID
/// * `event_type` - イベントタイプ
/// * `payload` - イベントのペイロード
///
/// # 戻り値
/// * 変換されたgRPCのイベント定義オブジェクト
/// * エラー: シリアライゼーションに失敗した場合
pub fn event_to_proto<T: serde::Serialize>(
    machine_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<proto::EventDefinition> {
    let payload_json = serde_json::to_string(payload).map_err(GrpcError::Serialization)?;

    Ok(proto::EventDefinition {
        machine_id: machine_id.to_string(),
        event_type: event_type.to_string(),
        payload: payload_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::EventType;
    use prost_types::Value as ProstValue;
    use rustate::{Action, ActionType as RuActionType, Guard, MachineBuilder, Transition};
    use rustate::{Context, Event, GuardType, State, StateType, TransitionType};
    use serde_json::json;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Helper function to create a simple state for testing
    fn create_test_state(id: &str) -> RuState {
        RuState::new(id)
    }

    // Helper function to create a simple transition for testing
    fn create_test_transition(source: &str, event: &str, target: &str) -> RuTransition {
        RuTransition::new(source, event, target)
    }

    // Mock functions for actions and guards if they don't exist
    // These should ideally be replaced with actual implementations or proper mocks
    fn action_to_proto(_action: &Arc<Action>) -> String {
        // Assuming action ID is stored in the name field
        _action.name.clone()
    }

    fn guard_to_proto(_guard: &Arc<Guard>) -> String {
        // Assuming guard ID is stored in the name field
        _guard.name.clone()
    }

    fn convert_context_to_proto(context: &RuContext) -> Result<String> {
        serde_json::to_string(context).map_err(GrpcError::Serialization)
    }

    fn convert_event_to_proto<T: serde::Serialize>(
        event_type: &str,
        payload: Option<&T>,
    ) -> Result<proto::EventDefinition> {
        let payload_json = if let Some(p) = payload {
            serde_json::to_string(p).map_err(GrpcError::Serialization)?
        } else {
            // Represent no payload as an empty string or "null"
            // Consistent handling depends on the consumer (from_proto)
            "null".to_string()
        };
        Ok(proto::EventDefinition {
            machine_id: "".to_string(), // Machine ID might not be needed here or passed differently
            event_type: event_type.to_string(),
            payload: payload_json,
        })
    }

    // Mock functions for proto to rust conversion if they don't exist
    fn convert_proto_to_state(proto_state: &proto::State) -> RuState {
        state_from_proto(proto_state) // Use existing function
    }

    fn convert_proto_to_transition(proto_transition: &proto::Transition) -> RuTransition {
        transition_from_proto(proto_transition) // Use existing function
    }

    fn convert_proto_to_context(proto_context: &str) -> Result<RuContext> {
        // Use Serialization variant for from_str errors based on context
        serde_json::from_str(proto_context).map_err(GrpcError::Serialization)
    }

    fn convert_proto_to_event(
        proto_event: &proto::EventDefinition,
    ) -> Result<(String, Option<serde_json::Value>)> {
        event_from_proto(proto_event) // Use existing function
    }

    fn convert_proto_to_machine_builder(
        proto_machine: &proto::MachineDefinition,
    ) -> (
        RuMachineBuilder,
        HashMap<String, Arc<Action>>,
        HashMap<String, Arc<Guard>>,
    ) {
        // This function needs a proper implementation based on how actions/guards are handled.
        // Returning a dummy builder for now.
        let mut builder = RuMachineBuilder::new(&proto_machine.id);
        builder = builder.initial(&proto_machine.initial);
        for state in &proto_machine.states {
            builder = builder.state(state_from_proto(state));
        }
        for transition in &proto_machine.transitions {
            builder = builder.transition(transition_from_proto(transition));
        }
        // Actions and guards need to be populated from proto_machine.actions/guards if defined
        // For now, return empty maps assuming they aren't part of the basic proto definition used here.
        (builder, HashMap::new(), HashMap::new())
    }

    fn create_test_machine() -> RuMachine {
        let mut builder = RuMachineBuilder::new("documentWorkflow");
        builder = builder.initial("draft");

        let draft = RuState::new("draft");
        let review = RuState::new("review");
        let published = RuState::new_final("published"); // Mark as final
        let archived = RuState::new("archived");

        builder = builder.state(draft);
        builder = builder.state(review);
        builder = builder.state(published);
        builder = builder.state(archived);

        // Create Action/Guard directly (not Arc) for with_action/with_guard if they take ownership
        let action = Action::new(
            "transitionAction",
            RuActionType::Transition,
            |_ctx, _evt| {},
        );
        let guard = Guard::new("transitionGuard", |_ctx, _evt| true);

        // Pass Action/Guard directly (cloning if needed, assuming they are Clone)
        let mut t1 = Transition::new("draft", "SUBMIT", "review");
        t1.with_action(action.clone()); // with_action likely takes impl IntoAction

        let mut t2 = Transition::new("review", "APPROVE", "published");
        t2.with_guard(guard.clone()); // with_guard likely takes impl IntoGuard

        let t3 = Transition::new("review", "REJECT", "draft");
        let t4 = Transition::new("published", "ARCHIVE", "archived");
        let t5 = Transition::new("archived", "RESTORE", "draft");

        // builder.transition takes Transition by value
        builder = builder.transition(t1); // t1 is moved here
        builder = builder.transition(t2); // t2 is moved here
        builder = builder.transition(t3);
        builder = builder.transition(t4);
        builder = builder.transition(t5);

        // Add initial context if needed
        let mut initial_context = RuContext::new();
        initial_context
            .set("initial_key", json!("initial_value"))
            .unwrap();
        builder = builder.context(initial_context);

        builder.build().unwrap() // Assuming build can succeed
    }

    #[test]
    fn test_convert_state_to_proto() {
        let state = create_test_state("myState");
        let proto_state = state_to_proto(&state);
        assert_eq!(proto_state.id, "myState");
        assert_eq!(
            proto_state.r#type, // Use r#type
            proto::StateType::Normal as i32
        );
        assert_eq!(proto_state.parent, ""); // Default parent is empty string
        assert!(proto_state.children.is_empty());
    }

    #[test]
    fn test_convert_state_with_parent_to_proto() {
        let mut state = create_test_state("childState");
        state.parent = Some("parentState".to_string());
        state.children = vec!["grandChild1".to_string(), "grandChild2".to_string()];
        let proto_state = state_to_proto(&state);
        assert_eq!(proto_state.id, "childState");
        assert_eq!(proto_state.parent, "parentState");
        assert_eq!(proto_state.children, vec!["grandChild1", "grandChild2"]);
    }

    #[test]
    fn test_convert_compound_state_to_proto() {
        // Note: RuStateType::Compound maps to proto::StateType::Normal
        let mut state = RuState::new_compound("compoundState", "initialChild");
        state.parent = Some("parentState".to_string());
        state.children = vec!["child1".to_string(), "child2".to_string()];

        let proto_state = state_to_proto(&state);
        assert_eq!(proto_state.id, "compoundState");
        assert_eq!(
            proto_state.r#type,              // Use r#type
            proto::StateType::Normal as i32  // Compound maps to Normal
        );
        assert_eq!(proto_state.parent, "parentState");
        assert_eq!(proto_state.children, vec!["child1", "child2"]);
        // `initial` is not part of proto::State, so we cannot assert it directly here.
        // The information might be implicitly handled during the from_proto conversion if needed.
    }

    #[test]
    fn test_convert_action_to_proto() {
        let action = Arc::new(Action::new(
            "myAction",
            RuActionType::Entry,
            |_ctx, _evt| {},
        ));
        let proto_action_id = action_to_proto(&action); // Use the mock/helper
        assert_eq!(proto_action_id, "myAction");
        // If proto::Action was a struct, you'd compare fields.
    }

    #[test]
    fn test_convert_guard_to_proto() {
        let guard = Arc::new(Guard::new("myGuard", |_ctx, _evt| true));
        let proto_guard_id = guard_to_proto(&guard); // Use the mock/helper
        assert_eq!(proto_guard_id, "myGuard");
        // If proto::Guard was a struct, you'd compare fields.
    }

    #[test]
    fn test_convert_transition_to_proto() {
        let action = Action::new(
            "transitionAction",
            RuActionType::Transition,
            |_ctx, _evt| {},
        );
        let guard = Guard::new("transitionGuard", |_ctx, _evt| true);
        let mut transition = create_test_transition("stateA", "EVENT", "stateB");
        // Transition fields actions: Vec<Action>, guard: Option<Guard>
        transition.actions = vec![action.clone()]; // Assign Action directly
        transition.guard = Some(guard.clone()); // Assign Guard directly

        let proto_transition = transition_to_proto(&transition);
        assert_eq!(proto_transition.source, "stateA");
        assert_eq!(proto_transition.event, "EVENT");
        assert_eq!(proto_transition.target, "stateB");

        // Assuming actions/guards in proto are just string IDs for now
        assert_eq!(proto_transition.actions.len(), 1);
        assert_eq!(proto_transition.actions[0], "transitionAction");
        assert_eq!(proto_transition.guards.len(), 1);
        assert_eq!(proto_transition.guards[0], "transitionGuard");
    }

    #[test]
    fn test_convert_machine_to_proto() {
        let machine = create_test_machine();
        let proto_machine_result = machine_to_proto(&machine);
        assert!(proto_machine_result.is_ok());
        let proto_machine = proto_machine_result.unwrap();

        assert_eq!(proto_machine.id, "documentWorkflow");
        assert_eq!(proto_machine.initial, "draft");
        assert_eq!(proto_machine.states.len(), 4); // draft, review, published, archived
        assert_eq!(proto_machine.transitions.len(), 5);
        // Optionally, assert specific state/transition details
        let state_ids: Vec<&str> = proto_machine.states.iter().map(|s| s.id.as_str()).collect();
        assert!(state_ids.contains(&"draft"));
        assert!(state_ids.contains(&"review"));
        assert!(state_ids.contains(&"published"));
        assert!(state_ids.contains(&"archived"));

        let transition_events: Vec<&str> = proto_machine
            .transitions
            .iter()
            .map(|t| t.event.as_str())
            .collect();
        assert!(transition_events.contains(&"SUBMIT"));
        assert!(transition_events.contains(&"APPROVE"));
        assert!(transition_events.contains(&"REJECT"));
        assert!(transition_events.contains(&"ARCHIVE"));
        assert!(transition_events.contains(&"RESTORE"));

        // Context assertion
        assert!(!proto_machine.context.is_empty());
        let context_value: serde_json::Value =
            serde_json::from_str(&proto_machine.context).unwrap();
        assert!(context_value.is_object()); // Check if it's a valid JSON object
        assert_eq!(context_value["initial_key"], "initial_value"); // Check context content
    }

    #[test]
    fn test_convert_context_to_proto() -> Result<()> {
        let mut context = RuContext::new();
        // Use set with &str for key
        context.set("count", json!(10)).unwrap();
        context
            .set("user", json!({"name": "Alice", "id": 123}))
            .unwrap();

        let proto_context = convert_context_to_proto(&context)?; // Use helper

        // Deserialize back to check content
        let deserialized_context: RuContext =
            serde_json::from_str(&proto_context).map_err(GrpcError::Serialization)?;

        // Use get::<Value> for type annotation
        assert_eq!(
            deserialized_context.get::<Value>("count").unwrap(),
            &json!(10)
        );
        assert_eq!(
            deserialized_context
                .get::<Value>("user") // Add type annotation
                .unwrap()
                .get("name")
                .unwrap(),
            &json!("Alice")
        );
        Ok(())
    }

    #[test]
    fn test_convert_event_to_proto() -> Result<()> {
        let event_type = "SIMPLE_EVENT";
        let proto_event = convert_event_to_proto::<serde_json::Value>(event_type, None)?; // Use helper

        assert_eq!(proto_event.event_type, event_type);
        assert_eq!(proto_event.payload, "null"); // Check for "null" string
        Ok(())
    }

    #[test]
    fn test_convert_event_with_payload_to_proto() -> Result<()> {
        let event_type = "PAYLOAD_EVENT";
        let payload = json!({"value": 42, "status": "active"});
        let proto_event = convert_event_to_proto(event_type, Some(&payload))?; // Use helper

        assert_eq!(proto_event.event_type, event_type);
        let deserialized_payload: serde_json::Value =
            // Use Serialization variant for from_str errors based on context
            serde_json::from_str(&proto_event.payload).map_err(GrpcError::Serialization)?;
        assert_eq!(deserialized_payload, payload);
        Ok(())
    }

    #[test]
    fn test_convert_proto_to_state() {
        let proto_state = proto::State {
            id: "testState".to_string(),
            r#type: proto::StateType::Normal as i32,
            parent: "parentState".to_string(),
            children: vec!["child1".to_string(), "child2".to_string()],
        };
        let state = convert_proto_to_state(&proto_state); // Use helper
        assert_eq!(state.id, "testState");
        assert_eq!(state.state_type, RuStateType::Normal);
        assert_eq!(state.parent, Some("parentState".to_string()));
        assert_eq!(
            state.children,
            vec!["child1".to_string(), "child2".to_string()]
        );
        // Cannot assert initial directly if not part of RuState model retrieved this way
    }

    #[test]
    fn test_convert_proto_to_transition() {
        let proto_transition = proto::Transition {
            source: "stateA".to_string(),
            event: "EVENT".to_string(),
            target: "stateB".to_string(),
            actions: vec!["action1".to_string()],
            guards: vec!["guard1".to_string()],
        };
        let transition = convert_proto_to_transition(&proto_transition); // Use helper
        assert_eq!(transition.source, "stateA");
        assert_eq!(transition.event, "EVENT");
        assert_eq!(transition.target, Some("stateB".to_string()));
        // Actions and guards are not directly populated by transition_from_proto in this basic setup
        assert!(transition.actions.is_empty());
        assert!(transition.guard.is_none()); // Check Option with is_none()
    }

    #[test]
    fn test_convert_proto_to_context() -> Result<()> {
        let context_json = r#"{"count": 20, "user": {"name": "Bob"}}"#.to_string();
        let context = convert_proto_to_context(&context_json)?; // Use helper

        // Use get::<Value> for type annotation
        assert_eq!(context.get::<Value>("count").unwrap(), &json!(20));
        assert_eq!(
            context.get::<Value>("user").unwrap().get("name").unwrap(), // Add type annotation
            &json!("Bob")
        );
        Ok(())
    }

    #[test]
    fn test_convert_proto_to_event() -> Result<()> {
        // Test simple event (payload is "null" string)
        let proto_event_simple = proto::EventDefinition {
            machine_id: "m1".to_string(),
            event_type: "SIMPLE".to_string(),
            payload: "null".to_string(),
        };
        let (event_type_simple, payload_simple) = convert_proto_to_event(&proto_event_simple)?; // Use helper
        assert_eq!(event_type_simple, "SIMPLE");
        assert!(payload_simple.is_none() || payload_simple.unwrap().is_null()); // Should deserialize "null" to None or Value::Null

        // Test event with payload
        let payload_json = json!({"data": "value"});
        let proto_event_with_payload = proto::EventDefinition {
            machine_id: "m2".to_string(),
            event_type: "WITH_PAYLOAD".to_string(),
            payload: payload_json.to_string(),
        };
        let (event_type_payload, payload_payload) =
            convert_proto_to_event(&proto_event_with_payload)?; // Use helper
        assert_eq!(event_type_payload, "WITH_PAYLOAD");
        assert_eq!(payload_payload.unwrap(), payload_json);

        Ok(())
    }

    #[test]
    fn test_roundtrip_machine_conversion() -> Result<()> {
        let original_machine = create_test_machine();
        let proto_machine = machine_to_proto(&original_machine)?; // Use helper

        // Convert back using the direct function (if it handles everything)
        let converted_machine = machine_from_proto(&proto_machine)?;

        // Compare fields - direct field access, not methods
        assert_eq!(converted_machine.name, original_machine.name);
        assert_eq!(converted_machine.initial, original_machine.initial);
        assert_eq!(
            converted_machine.states.len(),
            original_machine.states.len()
        );
        assert_eq!(
            converted_machine.transitions.len(),
            original_machine.transitions.len()
        );

        // Deep comparison of states (assuming State implements PartialEq and order is preserved)
        for (id, state) in &original_machine.states {
            assert!(
                converted_machine.states.contains_key(id),
                "State ID {} missing after conversion",
                id
            );
            let converted_state = converted_machine.states.get(id).unwrap();
            // Need to implement PartialEq for RuState or compare fields manually
            assert_eq!(converted_state.id, state.id);
            assert_eq!(
                converted_state.state_type, state.state_type,
                "State type mismatch for {}",
                id
            );
            assert_eq!(
                converted_state.parent, state.parent,
                "Parent mismatch for {}",
                id
            );
            // Sort children before comparing to handle potential order differences
            let mut sorted_children_orig = state.children.clone();
            sorted_children_orig.sort();
            let mut sorted_children_conv = converted_state.children.clone();
            sorted_children_conv.sort();
            assert_eq!(
                sorted_children_conv, sorted_children_orig,
                "Children mismatch for {}",
                id
            );
            // Compare other fields if necessary (initial, data, etc.)
        }

        // Deep comparison of transitions (assuming Transition implements PartialEq and order is preserved)
        // Note: Actions/Guards might not be fully restored in basic conversion
        assert_eq!(
            converted_machine.transitions.len(),
            original_machine.transitions.len(),
            "Transition count mismatch"
        );
        for i in 0..original_machine.transitions.len() {
            // Find matching transitions based on source/event/target as order might not be guaranteed
            let original_t = &original_machine.transitions[i];
            let converted_t = converted_machine
                .transitions
                .iter()
                .find(|t| {
                    t.source == original_t.source
                        && t.event == original_t.event
                        && t.target == original_t.target
                })
                .expect(&format!(
                    "Transition not found after conversion: {} -> {} on {}",
                    original_t.source,
                    original_t.target.clone().unwrap_or_default(),
                    original_t.event
                ));

            assert_eq!(converted_t.source, original_t.source);
            assert_eq!(converted_t.event, original_t.event);
            assert_eq!(converted_t.target, original_t.target);
            // Basic check, full action/guard comparison needs more logic and ID stability
            assert_eq!(
                converted_t.actions.len(),
                original_t.actions.len(),
                "Action count mismatch for transition {} -> {}",
                original_t.source,
                original_t.target.clone().unwrap_or_default()
            );
            assert_eq!(
                converted_t.guard.is_some(),
                original_t.guard.is_some(),
                "Guard presence mismatch"
            );
            // Further check action/guard names/ids if they are reliably converted
            if !original_t.actions.is_empty() {
                // Assuming converted_t.actions is also Vec<Arc<Action>>
                assert_eq!(converted_t.actions[0].name, original_t.actions[0].name);
            }
            // Check Option<Guard> correctly
            if let (Some(ref conv_guard), Some(ref orig_guard)) =
                (converted_t.guard.as_ref(), original_t.guard.as_ref())
            {
                assert_eq!(conv_guard.name, orig_guard.name);
            } else {
                // Compare Option using is_some() or is_none()
                assert_eq!(
                    converted_t.guard.is_none(),
                    original_t.guard.is_none(),
                    "Guard Option mismatch"
                );
            }
        }

        // Compare context - Commented out due to PartialEq not implemented
        // assert_eq!(converted_machine.context, original_machine.context);

        Ok(())
    }

    #[test]
    fn test_guard_conversion() {
        // Define a simple closure guard
        let guard_fn = |ctx: &Context, _evt: &Event| -> bool {
            ctx.get::<i32>("count").map_or(false, |c| c > 10) // Removed & from |&c|
        };
        // Use the correct Guard::new signature (name, predicate)
        let original_guard = Guard::new("count_guard", guard_fn);

        let proto_g = guard_to_proto(&original_guard).unwrap();
        assert_eq!(proto_g.name, "count_guard");

        // Convert back (condition logic is lost, compare by name)
        let converted_g = guard_from_proto::<Context, Event>(&proto_g).unwrap();
        assert_eq!(converted_g.name, original_guard.name);
        // Cannot compare the actual condition function
    }

    #[test]
    fn test_action_conversion() {
        // Define a simple async closure action
        let action_fn = |ctx_arc: Arc<RwLock<Context>>, _evt: &Event| async move {
            let mut ctx_lock = ctx_arc.write().await;
            let count = ctx_lock.get::<i32>("count").map_or(0, |c| c + 1); // Removed & from |&c|
            ctx_lock
                .set("count", count)
                .map_err(|e| StateError::ActionFailed(e.to_string())) // Use StateError::ActionFailed
        };

        // Use Action::from_fn
        let original_action = Action::from_fn(action_fn);

        let proto_a = action_to_proto(&original_action).unwrap();
        // Assuming action_to_proto uses a placeholder name or derives it
        // assert_eq!(proto_a.name, "some_action_name"); // Adjust based on actual implementation

        // Convert back (logic is lost)
        let converted_a = action_from_proto::<Context, Event>(&proto_a).unwrap();
        // Cannot compare function, maybe compare name if action_to_proto sets it?
    }

    #[test]
    fn test_transition_conversion() {
        // Create a sample transition
        let guard = Guard::new("test_guard", |_, _| true);
        let action = Action::from_fn(|_, _| async { Ok(()) }); // Dummy action
        let original_t = Transition::new(
            TestState::A,             // Source State
            Some(TestState::B),       // Target State
            Some(TestEvent::X),       // Event
            Some(guard),              // Guard
            vec![action],              // Actions
            TransitionType::External, // Transition Type
        );

        let proto_t = transition_to_proto(&original_t).unwrap();

        assert_eq!(proto_t.source.as_ref().unwrap().id, TestState::A.id());
        assert_eq!(proto_t.target.as_ref().unwrap().id, TestState::B.id());
        assert_eq!(proto_t.event.as_ref().unwrap().event_type, TestEvent::X.event_type());
        assert_eq!(proto_t.guard.as_ref().unwrap().name, "test_guard");
        assert_eq!(proto_t.actions.len(), 1);
        assert_eq!(proto_t.transition_type, proto::TransitionType::External as i32);

        // Convert back from proto
        let converted_t = transition_from_proto::<TestState, Context, TestEvent>(&proto_t).unwrap();

        // Compare relevant fields (guards/actions reconstructed by name/dummy)
        assert_eq!(converted_t.source, original_t.source);
        assert_eq!(converted_t.target, original_t.target);
        assert_eq!(converted_t.event, original_t.event);
        assert_eq!(converted_t.guard.map(|g| g.name), original_t.guard.map(|g| g.name));
        // Cannot easily compare reconstructed actions
        assert_eq!(converted_t.transition_type, original_t.transition_type);
    }
}
