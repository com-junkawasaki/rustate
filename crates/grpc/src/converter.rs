//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use crate::error::{ConversionError, GrpcError, Result};
use prost_types::Any;
use rustate::state::{State as RuState, StateType as RuStateType};
use rustate::transition::Transition as RuTransition;
use rustate::ActionType as RuActionType;
use rustate::{Context as RuContext, Machine as RuMachine, MachineBuilder as RuMachineBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value, Value};
use std::collections::HashMap;
use std::fmt::Debug;

// Import generated gRPC types
pub mod proto {
    tonic::include_proto!("rustate");
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
    transition::{Transition, TransitionType},
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
    S: StateTrait + DeserializeOwned + Clone + Send + Sync + 'static,
    C: DeserializeOwned + Clone + Send + Sync + Default + Debug + 'static,
    E: EventTrait + DeserializeOwned + Clone + Send + Sync + Eq + Debug + Serialize + 'static,
{
    let source = proto_transition
        .source
        .as_ref()
        .ok_or(ConversionError::MissingField(
            "transition source ".to_string(),
        ))
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
        .map(|a| Ok(Action::<C, E>::from_fn(|_, _| async { Ok(()) }))) // Reconstruct dummy action
        .collect::<Result<Vec<_>, ConversionError>>()?;

    let transition_type = proto::TransitionType::try_from(proto_transition.transition_type)
        .map_err(|e| {
            ConversionError::InvalidValue("invalid transition type ".to_string() + &e.to_string())
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
    C: Default + Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    Ok(Guard::new(&proto_guard.name, |_, _| true))
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
    C: Default + Send + Sync + 'static,
    E: Send + Sync + 'static,
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
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum TestState {
        A,
        B,
        C,
    }
    impl StateTrait for TestState {
        fn id(&self) -> String {
            format!("{:?}", self)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum TestEvent {
        X,
        Y,
    }
    impl EventTrait for TestEvent {
        fn event_type(&self) -> String {
            format!("{:?}", self)
        }
        fn payload(&self) -> Option<Value> {
            None
        } // Simple events, no payload
    }

    // Helper function to create a simple state for testing
    fn create_test_state(id: &str) -> State<TestState, Context, TestEvent> {
        // Use generic State
        State::new(TestState::A) // Need to create a variant
    }

    // Helper function to create a simple transition for testing
    fn create_test_transition(
        source: TestState,
        event: TestEvent,
        target: TestState,
    ) -> Transition<TestState, Context, TestEvent> {
        // Use generic Transition
        Transition::new(
            source,
            Some(target),
            Some(event),
            None,
            vec![],
            TransitionType::External,
        )
    }

    // Mock functions for actions and guards if they don't exist
    // These should ideally be replaced with actual implementations or proper mocks
    fn convert_context_to_proto(context: &RuContext) -> Result<String, ConversionError> {
        serde_json::to_string(context).map_err(ConversionError::Serialization)
    }

    // Use the main event_to_proto function
    // fn convert_event_to_proto<T: serde::Serialize>(
    //     event_type: &str,
    //     payload: Option<&T>,
    // ) -> Result<proto::EventDefinition> { ... }

    // Mock functions for proto to rust conversion if they don't exist
    fn convert_proto_to_state(proto_state: &proto::State) -> State<TestState, Context, TestEvent> {
        // Needs proper implementation or use state_from_proto
        State::new(TestState::A) // Dummy
    }

    fn convert_proto_to_transition(
        proto_transition: &proto::Transition,
    ) -> Transition<TestState, Context, TestEvent> {
        // Needs proper implementation or use transition_from_proto
        Transition::new(
            TestState::A,
            None,
            None,
            None,
            vec![],
            TransitionType::External,
        ) // Dummy
    }

    fn convert_proto_to_context(proto_context: &str) -> Result<RuContext, ConversionError> {
        serde_json::from_str(proto_context).map_err(ConversionError::Serialization)
    }

    // Use the main event_from_proto function
    // fn convert_proto_to_event(
    //     proto_event: &proto::EventDefinition,
    // ) -> Result<(String, Option<serde_json::Value>)> { ... }

    fn convert_proto_to_machine_builder(
        proto_machine: &proto::MachineDefinition,
    ) -> (
        // Specify generics for MachineBuilder
        MachineBuilder<TestState, TestEvent, Context>,
        HashMap<String, Arc<Action<Context, TestEvent>>>,
        HashMap<String, Arc<Guard<Context, TestEvent>>>,
    ) {
        // This function needs a proper implementation based on how actions/guards are handled.
        let mut builder = MachineBuilder::<TestState, TestEvent, Context>::new(TestState::A); // Dummy initial
                                                                                              // ... (rest remains dummy for now)
        (builder, HashMap::new(), HashMap::new())
    }

    fn create_test_machine() -> Machine<TestState, TestEvent, Context> {
        // Specify generics
        let mut builder = MachineBuilder::new(TestState::A);

        let draft = TestState::A; // Use enum variants
        let review = TestState::B;
        let published = TestState::C;
        // let archived = TestState::Archived; // Add if needed

        builder = builder.state(draft, |s| s); // Use state builder closure
        builder = builder.state(review, |s| s);
        builder = builder.state(published, |s| s.final_state(true)); // Mark as final
                                                                     // builder = builder.state(archived);

        let action = Action::new(
            "transitionAction",
            RuActionType::Transition,
            |_ctx, _evt| {},
        );
        let guard = Guard::new("transitionGuard", |_ctx, _evt| true);

        builder = builder.transition(Transition::new(
            draft,
            Some(review),
            Some(TestEvent::X),
            None,
            vec![action.clone()],
            TransitionType::External,
        ));
        builder = builder.transition(Transition::new(
            review,
            Some(published),
            Some(TestEvent::Y),
            Some(guard.clone()),
            vec![],
            TransitionType::External,
        ));
        builder = builder.transition(Transition::new(
            review,
            Some(draft),
            Some(TestEvent::X),
            None,
            vec![],
            TransitionType::External,
        ));
        // builder = builder.transition(Transition::new(published, "ARCHIVE", archived));
        // builder = builder.transition(Transition::new(archived, "RESTORE", draft));

        let mut initial_context = RuContext::new();
        initial_context
            .set("initial_key", json!("initial_value"))
            .unwrap();
        builder = builder.context(initial_context);

        builder.build().unwrap()
    }

    #[test]
    fn test_convert_state_to_proto() {
        let state = State::<TestState, Context, TestEvent>::new(TestState::A);
        let proto_state = state_to_proto(&state);
        assert_eq!(proto_state.id, "A"); // Enum variant name
        assert_eq!(proto_state.r#type, proto::StateType::Normal as i32);
        assert_eq!(proto_state.parent, "");
        assert!(proto_state.children.is_empty());
    }

    // ... Other tests need similar adjustments ...

    #[test]
    fn test_transition_conversion() {
        // Create a sample transition using TestState/TestEvent
        let guard = Guard::new("test_guard", |_: &Context, _: &TestEvent| true);
        // Simpler sync action for test setup
        let action = Action::new(
            "testAction",
            RuActionType::Transition,
            |_: &mut Context, _: &TestEvent| {},
        );

        let original_t = Transition::new(
            TestState::A,             // Source State
            Some(TestState::B),       // Target State
            Some(TestEvent::X),       // Event
            Some(guard.clone()),      // Guard
            vec![action.clone()],     // Actions
            TransitionType::External, // Transition Type
        );

        // ... (rest of the test, ensuring types match)
        let proto_t_result = transition_to_proto(&original_t);
        // ... (assertions remain similar, check IDs like "A", "B", "X")
        // ...
        let converted_t_result = transition_from_proto::<TestState, Context, TestEvent>(&proto_t);
        // ...
    }

    #[test]
    fn test_guard_conversion() {
        // Define a simple closure guard
        let guard_fn = |ctx: &Context, _evt: &TestEvent| -> bool {
            ctx.get::<i32>("count").map_or(false, |c| c > 10) // Removed & from |&c|
        };
        // Use the correct Guard::new signature (name, predicate)
        let original_guard = Guard::new("count_guard", guard_fn);

        // Use the moved function
        let proto_g = guard_to_proto(&original_guard).unwrap();
        assert_eq!(proto_g.name, "count_guard");

        // Convert back (condition logic is lost, compare by name)
        let converted_g = guard_from_proto::<Context, TestEvent>(&proto_g).unwrap();
        assert_eq!(converted_g.name, original_guard.name);
        // Cannot compare the actual condition function
    }

    #[test]
    fn test_action_conversion() {
        // Define a simple async closure action
        let action_fn = |ctx_arc: Arc<RwLock<Context>>, _evt: &TestEvent| async move {
            let mut ctx_lock = ctx_arc.write().await;
            let count = ctx_lock.get::<i32>("count").map_or(0, |c| c + 1); // Removed & from |&c|
            ctx_lock
                .set("count", count)
                .map_err(|e| StateError::ActionFailed(e.to_string())) // Use StateError::ActionFailed
        };

        // Use Action::from_fn
        let original_action = Action::from_fn(action_fn);

        // Use the moved function
        let proto_a = action_to_proto(&original_action).unwrap();
        // Assuming action_to_proto uses a placeholder name or derives it
        // assert_eq!(proto_a.name, "some_action_name"); // Adjust based on actual implementation

        // Convert back (logic is lost)
        let converted_a = action_from_proto::<Context, TestEvent>(&proto_a).unwrap();
        // Cannot compare function, maybe compare name if action_to_proto sets it?
    }
}
