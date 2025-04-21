//! RuStateとgRPC型間の変換を行うモジュール
//!
//! このモジュールは、RuStateのコアモデルとgRPC/Protocol Buffersの型の間で
//! 相互変換を行う機能を提供します。これにより、ネットワーク越しにステートマシンを
//! 安全かつ効率的に転送できます。

use rustate::{
    ActionType as RuActionType, Context as RuContext, 
    Machine as RuMachine, MachineBuilder as RuMachineBuilder,
    State as RuState, StateType as RuStateType, Transition as RuTransition,
};
use crate::proto;
use crate::error::{GrpcError, Result};
use std::convert::TryFrom;

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
    let state_type = state_type_from_proto(
        proto::StateType::try_from(proto_state.r#type)
            .unwrap_or(proto::StateType::Normal)
    );
    
    // 状態タイプに基づいて適切な状態を作成
    let mut state = match state_type {
        RuStateType::Normal => RuState::new(&proto_state.id),
        RuStateType::Final => RuState::new_final(&proto_state.id),
        RuStateType::Parallel => RuState::new_parallel(&proto_state.id),
        RuStateType::History => {
            // Historyは通常のStateTypeを使用して作成
            let s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::History;
            s
        },
        RuStateType::Compound => {
            // Compoundの場合は空の初期状態で作成（プロトコルからは情報が限られる）
            RuState::new_compound(&proto_state.id, "")
        },
        RuStateType::DeepHistory => {
            // DeepHistoryは通常のStateTypeを使用して作成
            let s = RuState::new(&proto_state.id);
            s.state_type = RuStateType::DeepHistory;
            s
        },
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
        id: transition.id.clone(),
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
    let mut transition = RuTransition::new(
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
    let context_json = serde_json::to_string(&machine.context)
        .map_err(GrpcError::Serialization)?;
    
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
        let context: RuContext = serde_json::from_str(&proto_machine.context)
            .map_err(GrpcError::Serialization)?;
        builder = builder.context(context);
    }
    
    // ステートマシンの構築
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
    let context_json = serde_json::to_string(&machine.context)
        .map_err(GrpcError::Serialization)?;
    
    // current_statesを文字列の配列として取得
    let current_states = machine.current_states.iter().map(|s| s.to_string()).collect();
    
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