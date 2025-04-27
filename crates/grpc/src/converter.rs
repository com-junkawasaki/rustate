//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use crate::error::{ConversionError, GrpcError, Result};
use prost_types::Any;
use rustate::state::{State as RuState, StateType as RuStateType};
use rustate::transition::{Transition as RuTransition, TransitionType as RuTransitionType};
use rustate::ActionType as RuActionType;
use rustate::{Context as RuContext, Machine as RuMachine, MachineBuilder as RuMachineBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value, Value};
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::Hash;

// Import generated gRPC types
pub mod proto {
    tonic::include_proto!("rustate");
}

// Import necessary types from rustate core
use rustate::{
    Action,
    Context,
    error::StateError,
    Event,
    EventTrait,
    Guard,
    IntoGuard,
    Machine,
    MachineBuilder,
    State,
    StateTrait,
    StateType,
    Transition,
    IntoAction,
};

// Import necessary types for JSON handling and conversion
use prost_types::{ListValue, Struct as ProstStruct, Value as ProstValue};
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
pub fn state_to_proto(state: &State) -> proto::State {
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
pub fn state_from_proto(proto_state: &proto::State) -> State {
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
pub fn transition_to_proto<C, E>(
    transition: &Transition,
) -> Result<proto::Transition>
where
    C: Serialize + Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Serialize + Send + Sync + 'static,
{
    // Convert source and target Option<String> (assuming S=String for simplicity now)
    // TODO: Revisit if S is not String. State IDs are likely strings.
    let source_id = proto::StateId { id: transition.source.to_string() };
    let target_id = transition.target.as_ref().map(|t| proto::StateId { id: t.to_string() });

    // Convert event Option<E> to String event type
    let event_type = match &transition.event {
        Some(e) => e.event_type().to_string(),
        None => "".to_string(),
    };

    // Convert guard Option<Guard> to Vec<string> (guard IDs/names)
    let guard_ids = match &transition.guard {
        Some(g) => vec![g.name.clone()],
        None => vec![],
    };

    // Convert actions Vec<Action> to Vec<string> (action IDs/names)
    let action_ids = transition
        .actions
        .iter()
        .map(|a| a.name.clone())
        .collect::<Vec<_>>();

    // TransitionType is not in proto, skip conversion
    // let transition_type = match transition.transition_type { ... }
    Ok(proto::Transition {
        source: source_id,
        event: event_type,
        target: target_id.unwrap_or_default(),
        guards: guard_ids,
        actions: action_ids,
        // transition_type field removed
    })
}

/// gRPCのTransitionからRuStateのTransitionへの変換
///
/// # 引数
/// * `proto_transition` - gRPCの遷移オブジェクト
///
/// # 戻り値
/// * 変換されたRuStateの遷移オブジェクト
pub fn transition_from_proto<C, E>(
    proto_transition: &proto::Transition,
) -> Result<Transition>
where
    C: DeserializeOwned + Clone + Send + Sync + Default + Debug + 'static,
    E: EventTrait + DeserializeOwned + Clone + Send + Sync + Eq + Debug + Serialize + 'static,
{
    let source_id = proto_transition.source.clone();

    // Target is string, convert to Option<String>
    let target_id = if proto_transition.target.is_empty() {
        None
    } else {
        Some(proto_transition.target.clone())
    };

    // Event is string (event type), need to reconstruct E if possible, or use a placeholder.
    // This depends heavily on E's definition. Using Default for now.
    // TODO: Implement proper event reconstruction based on proto_transition.event string.
    let event = E::default(); // Placeholder - Requires E: Default

    // Guards are strings (names), convert back to named Guard objects.
    let guard = if proto_transition.guards.is_empty() {
        None
    } else {
        // Assuming only one guard for simplicity, use the first name.
        // TODO: Handle multiple guards if necessary.
        Some(Guard::named(&proto_transition.guards[0]))
    };

    // Actions are strings (names), convert back to named Action objects.
    let actions = proto_transition
        .actions
        .iter()
        // Assuming Action::named exists or similar mechanism. Using default type for now.
        // TODO: Reconstruct actions properly based on name and potentially a registry.
        .map(|name| Action::named(name, RuActionType::Transition)) // Placeholder type
        .collect::<Vec<_>>();

    // TransitionType is not in proto, default to External
    let transition_type = TransitionType::External;

    // Assuming Transition::new takes String for source/target now
    Ok(Transition::new(&source_id, Some(event), target_id)
        .with_guard_opt(guard)
        .with_actions(actions)
        .with_type(transition_type))
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

/// Converts a Rust state ID (which implements StateTrait) to a proto::StateId (String).
/// Note: This assumes state IDs are represented as strings.
fn state_to_proto_id<S: StateTrait>(
    state: &S,
) -> std::result::Result<proto::StateId, ConversionError> {
    Ok(proto::StateId { id: state.id() })
}

/// Converts a proto::StateId (String) back to a Rust state type (which implements StateTrait).
/// Requires DeserializeOwned for S.
fn state_from_proto_id<S: StateTrait + DeserializeOwned>(
    proto_id: &proto::StateId,
) -> std::result::Result<S, ConversionError> {
    serde_json::from_str(&format!("\"{}\"", proto_id.id)).map_err(|e| {
        ConversionError::StateConversion(format!(
            "Failed to deserialize state from ID '{}': {}",
            proto_id.id, e
        ))
    })
}

/// Converts a Rust event (impl EventTrait) to proto::Event.
fn event_to_proto<E: EventTrait + Serialize>(
    event: &E,
) -> std::result::Result<proto::Event, ConversionError> {
    let payload_json =
        serde_json::to_string(&event.payload()).map_err(ConversionError::Serialization)?;
    Ok(proto::Event {
        event_type: event.event_type().to_string(),
        payload: payload_json,
    })
}

/// Converts a proto::Event back to a Rust event type (impl EventTrait).
fn event_from_proto<E: EventTrait + DeserializeOwned>(
    proto_event: &proto::Event,
) -> std::result::Result<E, ConversionError> {
    let payload_val: Value = serde_json::from_str(&proto_event.payload).unwrap_or(Value::Null);
    let json_obj = json!({
        "event_type": proto_event.event_type,
        "payload": payload_val
    });
    serde_json::from_value(json_obj).map_err(|e| {
        ConversionError::EventConversion(format!(
            "Failed to deserialize event '{}': {}",
            proto_event.event_type, e
        ))
    })
}

/// Converts a Rust Guard to a proto::Guard (assuming name is enough).
fn guard_to_proto<C, E>(guard: &Guard<C, E>) -> std::result::Result<proto::Guard, ConversionError> {
    Ok(proto::Guard {
        name: guard.name.clone(),
    })
}

/// Converts a proto::Guard back to a Rust Guard (reconstructing with a dummy condition).
fn guard_from_proto<C, E>(
    proto_guard: &proto::Guard,
) -> std::result::Result<Guard<C, E>, ConversionError>
where
    C: Default + Send + Sync + 'static + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
{
    Ok(Guard::named(&proto_guard.name))
}

/// Converts a Rust Action to a proto::Action (assuming name is enough).
fn action_to_proto<C, E>(
    action: &Action<C, E>,
) -> std::result::Result<proto::Action, ConversionError> {
    Ok(proto::Action {
        name: action.name.clone(),
        action_type: action_type_to_proto(&action.action_type) as i32,
    })
}

/// Converts a proto::Action back to a Rust Action (reconstructing with a dummy implementation).
fn action_from_proto<C, E>(
    proto_action: &proto::Action,
) -> std::result::Result<Action<C, E>, ConversionError>
where
    C: Default + Send + Sync + 'static + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
{
    let action_type = action_type_from_proto(
        proto::ActionType::try_from(proto_action.action_type)
            .unwrap_or(proto::ActionType::Transition),
    );
    Ok(Action::new(&proto_action.name, action_type, |_, _| {}))
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

    // Define simple TestState and TestEvent for testing
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
    enum TestState {
        #[default]
        A,
        B,
        C,
    }
    impl StateTrait for TestState {
        fn id(&self) -> &str {
            match self {
                TestState::A => "A",
                TestState::B => "B",
                TestState::C => "C",
            }
        }
        fn state_type(&self) -> StateType { StateType::Normal }
        fn parent(&self) -> Option<&str> { None }
        fn children(&self) -> &[String] { &[] }
        fn initial(&self) -> Option<&str> { None }
        fn data(&self) -> Option<&Value> { None }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum TestEvent {
        X,
        Y,
        Custom(String),
    }

    impl Default for TestEvent {
        fn default() -> Self { TestEvent::X }
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::X => "X",
                TestEvent::Y => "Y",
                TestEvent::Custom(_) => "Custom",
            }
        }
        fn payload(&self) -> Option<&Value> {
            match self {
                TestEvent::Custom(s) => {
                    // Cannot easily return &Value from temporary json!
                    // Need a different approach if payload reference is required.
                    // Returning None for now to satisfy trait signature.
                    None
                },
                _ => None,
            }
        }
    }

    // Helper function to create a simple state for testing
    fn create_test_state(id: TestState) -> State {
        State::new(id.id())
    }

    // Helper function to create a simple transition for testing
    fn create_test_transition(
        source: TestState,
        event: TestEvent,
        target: TestState,
    ) -> Transition {
        Transition::new(source.id(), Some(event), Some(target.id()))
            .with_type(TransitionType::External)
    }

    // Mock functions for actions and guards if they don't exist
    // These should ideally be replaced with actual implementations or proper mocks
    fn convert_context_to_proto(context: &RuContext) -> Result<String> {
        serde_json::to_string(&json!({})).map_err(|e| GrpcError::Serialization(e.to_string()))
    }

    // Mock functions for proto to rust conversion if they don't exist
    fn convert_proto_to_state(proto_state: &proto::State) -> State {
        state_from_proto(proto_state)
    }

    fn convert_proto_to_transition(
        proto_transition: &proto::Transition,
    ) -> Transition {
        transition_from_proto::<Context, TestEvent>(proto_transition).unwrap()
    }

    fn convert_proto_to_context(proto_context: &str) -> Result<RuContext> {
        if proto_context.is_empty() || proto_context == "{}" {
            Ok(Context::default())
        } else {
            Err(GrpcError::Deserialization(
                "Complex context deserialization not implemented".to_string(),
            ))
        }
    }

    fn convert_proto_to_machine_builder(
        proto_machine: &proto::MachineDefinition,
    ) -> (
        MachineBuilder<TestState, TestEvent>,
        HashMap<String, Arc<Action<Context, TestEvent>>>,
        HashMap<String, Arc<Guard<Context, TestEvent>>>,
    ) {
        let initial_state = TestState::A;
        let mut builder = MachineBuilder::<TestState, TestEvent>::new(
            &proto_machine.name,
            initial_state,
        );
        let actions = HashMap::new();
        let guards = HashMap::new();

        // TODO: Iterate through proto_machine.states, .transitions, etc.
        // - Create State objects and add to builder
        // - Create Transition objects (using named actions/guards) and add to builder
        // - Populate action/guard maps based on names found

        (builder, actions, guards)
    }

    fn create_test_machine() -> Machine<Context, TestEvent, TestState> {
        MachineBuilder::new("test", TestState::A)
            .state(State::new(TestState::A.id()))
            .state(State::new(TestState::B.id()))
            .transition(Transition::new(TestState::A.id(), Some(TestEvent::X), Some(TestState::B.id())))
            .build().unwrap()
    }

    #[test]
    fn test_state_type_conversion() {
        assert_eq!(
            state_type_to_proto(&RuStateType::Normal),
            proto::StateType::Normal
        );
        assert_eq!(
            state_type_from_proto(proto::StateType::Final),
            RuStateType::Final
        );
        // Add more cases
    }

    #[test]
    fn test_action_type_conversion() {
        assert_eq!(
            action_type_to_proto(&RuActionType::Entry),
            proto::ActionType::Entry
        );
        assert_eq!(
            action_type_from_proto(proto::ActionType::Exit),
            RuActionType::Exit
        );
    }

    #[test]
    fn test_convert_state_to_proto() {
        let state = create_test_state(TestState::A);
        let proto_state = state_to_proto(&state);
        assert_eq!(proto_state.id, "A");
        assert_eq!(proto_state.r#type, proto::StateType::Normal as i32);
    }

    #[test]
    fn test_convert_proto_to_state() {
        let proto_state = proto::State { id: "B".to_string(), r#type: proto::StateType::Normal as i32, parent: "".to_string(), children: vec![], initial_child: "".to_string() };
        let state = convert_proto_to_state(&proto_state);
        assert_eq!(state.id, "B");
        assert_eq!(state.state_type, StateType::Normal);
    }

    #[test]
    fn test_transition_conversion() {
        let transition = create_test_transition(TestState::A, TestEvent::X, TestState::B);
        let proto_transition_res = transition_to_proto::<Context, TestEvent>(&transition);
        assert!(proto_transition_res.is_ok());
        let proto_transition = proto_transition_res.unwrap();

        assert_eq!(proto_transition.source.unwrap().id, "A");
        assert_eq!(proto_transition.target.unwrap().id, "B");
        assert_eq!(proto_transition.event, "X");

        let converted_transition = convert_proto_to_transition(&proto_transition);
        assert_eq!(converted_transition.source, "A");
        assert_eq!(converted_transition.target.unwrap(), "B");
        assert_eq!(converted_transition.event, TestEvent::X);
    }

    #[test]
    fn test_guard_conversion() {
        let guard = Guard::<Context, TestEvent>::new("isReady", |_, _| true);
        let proto_guard_res = guard_to_proto(&guard);
        assert!(proto_guard_res.is_ok());
        let proto_guard = proto_guard_res.unwrap();
        assert_eq!(proto_guard.name, "isReady");

        let converted_guard_res = guard_from_proto::<Context, TestEvent>(&proto_guard);
        assert!(converted_guard_res.is_ok());
        let converted_guard = converted_guard_res.unwrap();
        assert_eq!(converted_guard.name, "isReady");
    }

    #[test]
    fn test_action_conversion() {
        let action = Action::<Context, TestEvent>::new("doSomething", RuActionType::Entry, |_, _| {});
        let proto_action_res = action_to_proto(&action);
        assert!(proto_action_res.is_ok());
        let proto_action = proto_action_res.unwrap();
        assert_eq!(proto_action.name, "doSomething");
        assert_eq!(proto_action.r#type, proto::ActionType::Entry as i32);

        let converted_action_res = action_from_proto::<Context, TestEvent>(&proto_action);
        assert!(converted_action_res.is_ok());
        let converted_action = converted_action_res.unwrap();
        assert_eq!(converted_action.name, "doSomething");
    }

    #[test]
    fn test_event_conversion_with_payload() {
        let original_event = TestEvent::Custom("hello".to_string());
        let proto_event_res = event_to_proto(&original_event);
        assert!(proto_event_res.is_ok());
        let proto_event = proto_event_res.unwrap();

        assert_eq!(proto_event.event_type, "Custom");
        assert!(!proto_event.payload.is_empty());

        let converted_event_res = event_from_proto::<TestEvent>(&proto_event);
        assert!(converted_event_res.is_ok());
        let converted_event = converted_event_res.unwrap();

        assert_eq!(converted_event, original_event);
    }

    #[test]
    fn test_event_conversion_without_payload() {
        let original_event = TestEvent::X;
        let proto_event_res = event_to_proto(&original_event);
        assert!(proto_event_res.is_ok());
        let proto_event = proto_event_res.unwrap();

        assert_eq!(proto_event.event_type, "X");
        assert!(proto_event.payload.is_empty());

        let converted_event_res = event_from_proto::<TestEvent>(&proto_event);
        assert!(converted_event_res.is_ok());
        let converted_event = converted_event_res.unwrap();

        assert_eq!(converted_event, original_event);
    }
}
