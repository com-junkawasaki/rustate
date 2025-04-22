//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use crate::error::{GrpcError, Result};
use crate::proto;
use rustate::state::{State as RuState, StateType as RuStateType};
use rustate::transition::Transition as RuTransition;
use rustate::ActionType as RuActionType;
use rustate::{Context as RuContext, Machine as RuMachine, MachineBuilder as RuMachineBuilder};

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
    let state_type = state_type_from_proto(match proto_state.r#type {
        0 => proto::StateType::Normal,
        1 => proto::StateType::Final,
        2 => proto::StateType::History,
        3 => proto::StateType::Parallel,
        _ => proto::StateType::Normal, // Default to Normal for unknown values
    });

    let mut state = match state_type {
        RuStateType::Normal => RuState::new(&proto_state.id),
        RuStateType::Final => RuState::new_final(&proto_state.id),
        RuStateType::History => {
            // ヒストリーは通常のStateTypeを使用して作成
            let mut s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::History;
            s
        }
        RuStateType::DeepHistory => {
            // DeepHistoryは通常のStateTypeを使用して作成
            let mut s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::DeepHistory;
            s
        }
        RuStateType::Parallel => RuState::new_parallel(&proto_state.id),
        RuStateType::Compound => {
            // Compoundの場合は空の初期状態で作成（プロトコルからは情報が限られる）
            RuState::new_compound(&proto_state.id, "")
        }
    };

    // 親状態の設定（存在する場合）
    if !proto_state.parent.is_empty() {
        state.parent = Some(proto_state.parent.clone());
    }

    // 子状態の追加
    if !proto_state.children.is_empty() {
        for child in &proto_state.children {
            state.children.push(child.clone());
        }
    }

    state
}

/// RuStateのTransitionからgRPCのTransitionへの変換
///
/// # 引数
/// * `transition` - RuStateの遷移オブジェクト
///
/// # 戻り値
/// * 変換されたgRPCの遷移オブジェクト
pub fn transition_to_proto(transition: &RuTransition) -> proto::Transition {
    proto::Transition {
        source: transition.source.clone(),
        event: transition.event.clone(),
        target: transition.target.clone().unwrap_or_default().to_string(),
        guards: transition.guard.iter().map(|g| g.to_string()).collect(),
        actions: transition.actions.iter().map(|a| a.to_string()).collect(),
    }
}

/// gRPCのTransitionからRuStateのTransitionへの変換
///
/// # 引数
/// * `proto_transition` - gRPCの遷移オブジェクト
///
/// # 戻り値
/// * 変換されたRuStateの遷移オブジェクト
pub fn transition_from_proto(proto_transition: &proto::Transition) -> RuTransition {
    let transition = RuTransition::new(
        &proto_transition.source,
        &proto_transition.event,
        &proto_transition.target,
    );

    // ガードとアクションの設定
    // 注意: 実際のrustateのTransitionにはこれらを直接設定する方法がないため、
    // 単純なクローンだけを行う (実際のアクションやガードの設定はMachineBuilder経由で行われる)

    transition
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
        transitions.push(transition_to_proto(transition));
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
    for state in &proto_machine.states {
        builder = builder.state(state_from_proto(state));
    }

    // 遷移の追加
    for transition in &proto_machine.transitions {
        builder = builder.transition(transition_from_proto(transition));
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
/// * イベント文字列
pub fn event_from_proto(proto_event: &proto::EventDefinition) -> Result<String> {
    Ok(proto_event.event_type.clone())
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
    use crate::error::GrpcError;
    use crate::proto;
    use rustate::{Action, ActionType, Guard, Machine, MachineBuilder, State, Transition};
    use std::collections::HashMap;
    use uuid::Uuid;

    // ステートマシンのテスト用ヘルパー関数
    fn create_test_machine() -> Machine {
        // 状態定義
        let draft_state = State::new("draft");
        let review_state = State::new("review");
        let published_state = State::new("published");
        let archived_state = State::new("archived");

        // 遷移の定義
        let submit_transition = Transition::new("draft", "SUBMIT", "review");
        let approve_transition = Transition::new("review", "APPROVE", "published");
        let reject_transition = Transition::new("review", "REJECT", "draft");
        let archive_transition = Transition::new("published", "ARCHIVE", "archived");
        let restore_transition = Transition::new("archived", "RESTORE", "draft");

        // 状態マシンの構築
        let machine = MachineBuilder::new("documentWorkflow")
            .state(draft_state)
            .state(review_state)
            .state(published_state)
            .state(archived_state)
            .initial("draft")
            .transition(submit_transition)
            .transition(approve_transition)
            .transition(reject_transition)
            .transition(archive_transition)
            .transition(restore_transition)
            .build()
            .unwrap();

        machine
    }

    #[test]
    fn test_convert_state_to_proto() {
        let state = State::new("testState");
        let proto_state = convert_state_to_proto(&state);

        assert_eq!(proto_state.id, "testState");
        assert_eq!(proto_state.state_type, proto::StateType::Simple as i32);
        assert!(proto_state.parent.is_none());
        assert_eq!(proto_state.children.len(), 0);
        assert!(proto_state.initial.is_none());
        assert!(proto_state.data.is_none());
    }

    #[test]
    fn test_convert_state_with_parent_to_proto() {
        let mut state = State::new("childState");
        state.with_parent("parentState");
        let proto_state = convert_state_to_proto(&state);

        assert_eq!(proto_state.id, "childState");
        assert_eq!(proto_state.parent.unwrap(), "parentState");
    }

    #[test]
    fn test_convert_compound_state_to_proto() {
        let mut state = State::new("compoundState");
        state.with_type(rustate::StateType::Compound);
        state.with_initial("initialChild");
        state.with_children(vec!["child1".to_string(), "child2".to_string()]);
        
        let proto_state = convert_state_to_proto(&state);

        assert_eq!(proto_state.id, "compoundState");
        assert_eq!(proto_state.state_type, proto::StateType::Compound as i32);
        assert_eq!(proto_state.initial.unwrap(), "initialChild");
        assert_eq!(proto_state.children, vec!["child1", "child2"]);
    }

    #[test]
    fn test_convert_action_to_proto() {
        let action = Action::new("testAction", ActionType::Entry, |_ctx, _evt| {
            // 空の実装
        });

        let proto_action = convert_action_to_proto(&action);

        assert_eq!(proto_action.id, "testAction");
        assert_eq!(proto_action.action_type, proto::ActionType::Entry as i32);
    }

    #[test]
    fn test_convert_guard_to_proto() {
        let guard = Guard::new("testGuard", |_ctx, _evt| true);
        let proto_guard = convert_guard_to_proto(&guard);

        assert_eq!(proto_guard.id, "testGuard");
    }

    #[test]
    fn test_convert_transition_to_proto() {
        let mut transition = Transition::new("sourceState", "EVENT_TYPE", "targetState");
        let action = Action::new("transitionAction", ActionType::Transition, |_ctx, _evt| {
            // 空の実装
        });
        let guard = Guard::new("transitionGuard", |_ctx, _evt| true);

        transition.with_action(action);
        transition.with_guard(guard);

        let proto_transition = convert_transition_to_proto(&transition);

        assert_eq!(proto_transition.source, "sourceState");
        assert_eq!(proto_transition.event, "EVENT_TYPE");
        assert_eq!(proto_transition.target, "targetState");
        assert_eq!(proto_transition.actions.len(), 1);
        assert_eq!(proto_transition.actions[0].id, "transitionAction");
        assert_eq!(proto_transition.guards.len(), 1);
        assert_eq!(proto_transition.guards[0].id, "transitionGuard");
    }

    #[test]
    fn test_convert_machine_to_proto() {
        let machine = create_test_machine();
        let proto_machine = convert_machine_to_proto(&machine);

        assert_eq!(proto_machine.id, "documentWorkflow");
        assert_eq!(proto_machine.initial, "draft");
        assert_eq!(proto_machine.states.len(), 4);
        assert_eq!(proto_machine.transitions.len(), 5);

        // 状態のIDを確認
        let state_ids: Vec<&str> = proto_machine.states.iter().map(|s| s.id.as_str()).collect();
        assert!(state_ids.contains(&"draft"));
        assert!(state_ids.contains(&"review"));
        assert!(state_ids.contains(&"published"));
        assert!(state_ids.contains(&"archived"));

        // 遷移のソースとターゲットの関係を確認
        let transitions: Vec<(&str, &str, &str)> = proto_machine
            .transitions
            .iter()
            .map(|t| (t.source.as_str(), t.event.as_str(), t.target.as_str()))
            .collect();

        assert!(transitions.contains(&("draft", "SUBMIT", "review")));
        assert!(transitions.contains(&("review", "APPROVE", "published")));
        assert!(transitions.contains(&("review", "REJECT", "draft")));
        assert!(transitions.contains(&("published", "ARCHIVE", "archived")));
        assert!(transitions.contains(&("archived", "RESTORE", "draft")));
    }

    #[test]
    fn test_convert_context_to_proto() {
        let mut context = rustate::Context::new();
        context.set("stringKey", "stringValue").unwrap();
        context.set("intKey", 42).unwrap();
        context.set("boolKey", true).unwrap();
        
        let proto_context = convert_context_to_proto(&context);
        
        // プロトコル値が正しく変換されていることを確認
        assert_eq!(proto_context.values.len(), 3);
        
        // マップに変換して簡単な検証
        let value_map: HashMap<&str, &proto::Value> = proto_context
            .values
            .iter()
            .map(|v| (v.key.as_str(), &v.value.as_ref().unwrap()))
            .collect();
        
        // 文字列値の検証
        let string_value = value_map.get("stringKey").unwrap();
        assert_eq!(string_value.kind, Some(proto::value::Kind::StringValue("stringValue".to_string())));
        
        // 整数値の検証
        let int_value = value_map.get("intKey").unwrap();
        assert_eq!(int_value.kind, Some(proto::value::Kind::IntValue(42)));
        
        // ブール値の検証
        let bool_value = value_map.get("boolKey").unwrap();
        assert_eq!(bool_value.kind, Some(proto::value::Kind::BoolValue(true)));
    }

    #[test]
    fn test_convert_event_to_proto() {
        let event = rustate::Event::new("TEST_EVENT");
        let proto_event = convert_event_to_proto(&event);
        
        assert_eq!(proto_event.event_type, "TEST_EVENT");
        assert!(proto_event.payload.is_none());
    }

    #[test]
    fn test_convert_event_with_payload_to_proto() {
        let mut event = rustate::Event::new("TEST_EVENT");
        let mut payload = serde_json::Map::new();
        payload.insert("key".to_string(), serde_json::Value::String("value".to_string()));
        let json_payload = serde_json::Value::Object(payload);
        event.with_payload(json_payload);
        
        let proto_event = convert_event_to_proto(&event);
        
        assert_eq!(proto_event.event_type, "TEST_EVENT");
        assert!(proto_event.payload.is_some());
        
        let payload_json = proto_event.payload.unwrap().json;
        assert!(payload_json.contains("\"key\":\"value\""));
    }

    #[test]
    fn test_convert_proto_to_state() {
        let mut proto_state = proto::State {
            id: "testState".to_string(),
            state_type: proto::StateType::Compound as i32,
            parent: Some("parentState".to_string()),
            children: vec!["child1".to_string(), "child2".to_string()],
            initial: Some("child1".to_string()),
            data: None,
        };
        
        let state = convert_proto_to_state(&proto_state);
        
        assert_eq!(state.id(), "testState");
        assert_eq!(state.state_type(), &rustate::StateType::Compound);
        assert_eq!(state.parent(), Some("parentState"));
        assert_eq!(state.children(), &["child1", "child2"]);
        assert_eq!(state.initial(), Some("child1"));
    }

    #[test]
    fn test_convert_proto_to_transition() {
        let proto_transition = proto::Transition {
            source: "sourceState".to_string(),
            event: "EVENT_TYPE".to_string(),
            target: "targetState".to_string(),
            actions: Vec::new(),
            guards: Vec::new(),
        };
        
        let transition = convert_proto_to_transition(&proto_transition);
        
        assert_eq!(transition.source(), "sourceState");
        assert_eq!(transition.event(), "EVENT_TYPE");
        assert_eq!(transition.target(), "targetState");
        assert!(transition.actions().is_empty());
        assert!(transition.guards().is_empty());
    }

    #[test]
    fn test_convert_proto_to_context() -> Result<(), GrpcError> {
        // プロトコルコンテキストを作成
        let mut proto_context = proto::Context { values: Vec::new() };
        
        // 文字列値
        let string_value = proto::Value {
            kind: Some(proto::value::Kind::StringValue("stringValue".to_string())),
        };
        proto_context.values.push(proto::KeyValue {
            key: "stringKey".to_string(),
            value: Some(string_value),
        });
        
        // 整数値
        let int_value = proto::Value {
            kind: Some(proto::value::Kind::IntValue(42)),
        };
        proto_context.values.push(proto::KeyValue {
            key: "intKey".to_string(),
            value: Some(int_value),
        });
        
        // ブール値
        let bool_value = proto::Value {
            kind: Some(proto::value::Kind::BoolValue(true)),
        };
        proto_context.values.push(proto::KeyValue {
            key: "boolKey".to_string(),
            value: Some(bool_value),
        });
        
        let context = convert_proto_to_context(&proto_context)?;
        
        // 変換されたコンテキストの値を検証
        assert_eq!(context.get::<String>("stringKey")?, "stringValue");
        assert_eq!(context.get::<i64>("intKey")?, 42);
        assert_eq!(context.get::<bool>("boolKey")?, true);
        
        Ok(())
    }

    #[test]
    fn test_convert_proto_to_event() -> Result<(), GrpcError> {
        // ペイロードなしのイベント
        let proto_event_simple = proto::Event {
            event_type: "SIMPLE_EVENT".to_string(),
            payload: None,
        };
        
        let event_simple = convert_proto_to_event(&proto_event_simple)?;
        assert_eq!(event_simple.event_type(), "SIMPLE_EVENT");
        assert!(event_simple.payload().is_none());
        
        // ペイロード付きのイベント
        let proto_payload = proto::Payload {
            json: r#"{"key":"value","number":42}"#.to_string(),
        };
        
        let proto_event_with_payload = proto::Event {
            event_type: "PAYLOAD_EVENT".to_string(),
            payload: Some(proto_payload),
        };
        
        let event_with_payload = convert_proto_to_event(&proto_event_with_payload)?;
        assert_eq!(event_with_payload.event_type(), "PAYLOAD_EVENT");
        
        // ペイロードのデシリアライズを検証
        let payload = event_with_payload.payload().unwrap();
        if let serde_json::Value::Object(map) = payload {
            assert_eq!(map["key"], "value");
            assert_eq!(map["number"], 42);
        } else {
            panic!("Expected Object payload");
        }
        
        Ok(())
    }

    #[test]
    fn test_roundtrip_machine_conversion() {
        // オリジナルのマシンを作成
        let original_machine = create_test_machine();
        
        // マシンをプロトコルに変換
        let proto_machine = convert_machine_to_proto(&original_machine);
        
        // プロトコルからマシンを再構築
        let (builder, _) = convert_proto_to_machine_builder(&proto_machine);
        let converted_machine = builder.build().unwrap();
        
        // 変換前後でプロパティが保持されていることを確認
        assert_eq!(converted_machine.id(), original_machine.id());
        assert_eq!(converted_machine.initial(), original_machine.initial());
        
        // 状態の数が一致することを確認
        assert_eq!(
            converted_machine.states().len(),
            original_machine.states().len()
        );
        
        // 遷移の数が一致することを確認
        assert_eq!(
            converted_machine.transitions().len(),
            original_machine.transitions().len()
        );
        
        // 各状態のIDが保持されていることを確認
        for state in original_machine.states() {
            let state_id = state.id();
            assert!(
                converted_machine.states().iter().any(|s| s.id() == state_id),
                "State {} not found in converted machine",
                state_id
            );
        }
        
        // 各遷移のソース/イベント/ターゲットが保持されていることを確認
        for transition in original_machine.transitions() {
            let source = transition.source();
            let event = transition.event();
            let target = transition.target();
            
            assert!(
                converted_machine.transitions().iter().any(|t| 
                    t.source() == source && t.event() == event && t.target() == target
                ),
                "Transition {}--{}-->{} not found in converted machine",
                source, event, target
            );
        }
    }
}
