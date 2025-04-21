//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use rustate::{
    Action as RuAction, ActionType as RuActionType, Context as RuContext, 
    Guard as RuGuard, Machine as RuMachine, MachineBuilder as RuMachineBuilder,
    State as RuState, StateType as RuStateType, Transition as RuTransition
};
use crate::proto;
use crate::error::{GrpcError, Result};

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
        id: state.id().to_string(),
        r#type: state_type_to_proto(state.state_type()) as i32,
        parent: state.parent().unwrap_or_default().to_string(),
        children: state.children().iter().map(|s| s.to_string()).collect(),
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
    let state_type = state_type_from_proto(proto::StateType::from_i32(proto_state.r#type)
        .unwrap_or(proto::StateType::Normal));
    
    let mut state = RuState::new_with_type(&proto_state.id, state_type);
    
    if !proto_state.parent.is_empty() {
        state.set_parent(&proto_state.parent);
    }
    
    // 子ノードの追加を一度の呼び出しでまとめて行う最適化
    if !proto_state.children.is_empty() {
        for child in &proto_state.children {
            state.add_child(child);
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
        source: transition.source().to_string(),
        event: transition.event().to_string(),
        target: transition.target().to_string(),
        guards: transition.guards().iter().map(|g| g.to_string()).collect(),
        actions: transition.actions().iter().map(|a| a.to_string()).collect(),
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
    let mut transition = RuTransition::new(
        &proto_transition.source,
        &proto_transition.event,
        &proto_transition.target,
    );
    
    // 値がある場合のみ処理（最適化）
    if !proto_transition.guards.is_empty() {
        for guard in &proto_transition.guards {
            transition.add_guard(guard);
        }
    }
    
    if !proto_transition.actions.is_empty() {
        for action in &proto_transition.actions {
            transition.add_action(action);
        }
    }
    
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
    // 状態の変換を事前に一括で行う（最適化）
    let states: Vec<proto::State> = machine
        .states()
        .into_iter()
        .map(state_to_proto)
        .collect();
    
    // 遷移の変換を事前に一括で行う（最適化）
    let transitions: Vec<proto::Transition> = machine
        .transitions()
        .into_iter()
        .map(transition_to_proto)
        .collect();
    
    // コンテキストのJSONシリアライズ
    let context_json = serde_json::to_string(machine.context())
        .map_err(GrpcError::Serialization)?;
    
    Ok(proto::MachineDefinition {
        id: machine.id().to_string(),
        initial: machine.initial().to_string(),
        states,
        transitions,
        // アクションとガードの詳細情報はプロトタイプとして簡略化
        // 将来的に完全なシリアライズ/デシリアライズを実装予定
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
    
    // 状態の追加（最適化: キャパシティを事前に確保）
    let states_capacity = proto_machine.states.len();
    for state in &proto_machine.states {
        builder = builder.state(state_from_proto(state));
    }
    
    // 遷移の追加
    for transition in &proto_machine.transitions {
        builder = builder.transition(transition_from_proto(transition));
    }
    
    // コンテキストの設定（存在する場合のみ処理）
    if !proto_machine.context.is_empty() {
        let context: RuContext = serde_json::from_str(&proto_machine.context)
            .map_err(GrpcError::Serialization)?;
        builder = builder.context(context);
    }
    
    // 注: アクションとガードの詳細なインポートは今後実装予定
    
    let machine = builder.build()
        .map_err(GrpcError::StateMachine)?;
    
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
    let context_json = serde_json::to_string(machine.context())
        .map_err(GrpcError::Serialization)?;
    
    Ok(proto::MachineState {
        machine_id: machine.id().to_string(),
        current_states: machine.current_states().iter().map(|s| s.to_string()).collect(),
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
    payload: &T
) -> Result<proto::EventDefinition> {
    let payload_json = serde_json::to_string(payload)
        .map_err(GrpcError::Serialization)?;
    
    Ok(proto::EventDefinition {
        machine_id: machine_id.to_string(),
        event_type: event_type.to_string(),
        payload: payload_json,
    })
} 