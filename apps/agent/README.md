# Rustate Agent

Rustate Agent は、Rust で実装された状態機械駆動の LLM エージェントフレームワークです。このライブラリは [statelyai/agent](https://github.com/statelyai/agent) を参考に開発されており、Rust の型システムと所有権モデルを活かした安全で効率的な実装を提供します。

## 特徴

- **状態機械駆動**: rustateを使用した明示的な状態遷移とエージェント行動の制御
- **観測と学習**: エージェントの行動と結果を記録し、将来の意思決定に活用
- **フィードバックの統合**: エージェントのパフォーマンスを評価し、行動を改善するためのフィードバック機構
- **非同期サポート**: tokioを使用した効率的な非同期処理
- **安全性重視**: Rustの型システムと所有権モデルを活かした安全な実装
- **共有状態と統合**: rustateの統合機能を活用した複数エージェント間の連携
- **分散エージェント**: 複数のエージェントが協調して問題を解決する機構

## 主要コンセプト

- **観測 (Observations)**: 状態遷移の記録（前の状態、アクション、結果の状態、メタデータ）
- **決定 (Decisions)**: エージェントが選択するアクション（現在の状態、目標状態、過去の観測に基づく）
- **フィードバック (Feedback)**: 決定に対する評価や報酬
- **洞察 (Insights)**: 状態遷移に関する追加コンテキスト
- **エピソード (Episodes)**: 初期状態から目標状態までの完全な状態遷移シーケンス
- **共有コンテキスト (Shared Context)**: 複数のエージェント間で共有されるデータ
- **共有状態機械 (Shared Machines)**: 複数のエージェントから操作可能な状態機械

## 使用例

```rust
use rustate::{Machine, MachineBuilder, State, Transition, Context};
use rustate_agent::{
    agent::Agent,
    policy::RuleBasedPolicy,
    storage::MemoryStorage,
    decision::Decision,
};

// 状態とイベントを定義
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum TaskState {
    Idle,
    Processing,
    Completed,
    Error,
}

impl rustate::StateTrait for TaskState {
    fn get_id(&self) -> String {
        match self {
            TaskState::Idle => "idle".to_string(),
            TaskState::Processing => "processing".to_string(),
            TaskState::Completed => "completed".to_string(),
            TaskState::Error => "error".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum TaskEvent {
    Start,
    Process,
    Complete,
    Fail,
}

impl rustate::EventTrait for TaskEvent {
    fn event_type(&self) -> String {
        match self {
            TaskEvent::Start => "START".to_string(),
            TaskEvent::Process => "PROCESS".to_string(),
            TaskEvent::Complete => "COMPLETE".to_string(),
            TaskEvent::Fail => "FAIL".to_string(),
        }
    }
}

impl rustate::IntoEvent for TaskEvent {
    fn into_event(self) -> rustate::Event {
        rustate::Event::new(self.event_type())
    }
}

// 状態機械を作成
fn create_task_machine() -> Machine<TaskState, TaskEvent> {
    let idle = State::new("idle", TaskState::Idle);
    let processing = State::new("processing", TaskState::Processing);
    let completed = State::new("completed", TaskState::Completed);
    let error = State::new("error", TaskState::Error);

    let idle_to_processing = Transition::new("idle", "START", "processing");
    let processing_to_completed = Transition::new("processing", "COMPLETE", "completed");
    let processing_to_error = Transition::new("processing", "FAIL", "error");
    let error_to_processing = Transition::new("error", "START", "processing");

    MachineBuilder::new("task_machine")
        .state(idle)
        .state(processing)
        .state(completed)
        .state(error)
        .initial("idle")
        .transition(idle_to_processing)
        .transition(processing_to_completed)
        .transition(processing_to_error)
        .transition(error_to_processing)
        .build()
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ポリシーを定義
    let policy = RuleBasedPolicy::new()
        .add_rule_for_state(TaskState::Idle, TaskEvent::Start, 0.9)
        .add_rule_for_state(TaskState::Processing, TaskEvent::Complete, 0.8)
        .add_rule_for_state(TaskState::Error, TaskEvent::Start, 0.7)
        .with_fallback(TaskEvent::Process);

    // ストレージを作成
    let storage = MemoryStorage::new();

    // 状態機械を作成
    let machine = create_task_machine();

    // エージェントを作成
    let mut agent = Agent::new(machine, policy, storage);

    // エピソードを開始
    agent.start_episode("タスク実行", Some(TaskState::Completed)).await?;

    // 目標達成まで実行
    let success = agent.run_until_goal(Some(5)).await?;
    println!("エピソード完了: {}", if success { "成功" } else { "失敗" });

    Ok(())
}
```

## 共有機能を使用した例

```rust
use rustate::{Machine, MachineBuilder, State, Transition};
use rustate::integration::{SharedMachineRef, SharedContext};
use rustate_agent::{
    agent::Agent,
    policy::RuleBasedPolicy,
    storage::MemoryStorage,
};

// ... 状態とイベントの定義は上の例と同じ ...

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 状態機械を作成
    let machine = create_task_machine();
    
    // 共有状態機械参照を作成
    let shared_machine = SharedMachineRef::new(machine);
    
    // 共有コンテキストを作成
    let shared_context = SharedContext::new();
    
    // エージェント1を作成 (共有状態機械を使用)
    let policy1 = RuleBasedPolicy::new()
        .add_rule_for_state(TaskState::Idle, TaskEvent::Start, 0.9)
        .with_fallback(TaskEvent::Process);
    let storage1 = MemoryStorage::new();
    let mut agent1 = Agent::with_shared_machine(
        shared_machine.clone(),
        policy1,
        storage1
    ).with_shared_context(shared_context.clone());

    // エージェント2を作成 (共有状態機械を使用)
    let policy2 = RuleBasedPolicy::new()
        .add_rule_for_state(TaskState::Processing, TaskEvent::Complete, 0.9)
        .with_fallback(TaskEvent::Process);
    let storage2 = MemoryStorage::new();
    let mut agent2 = Agent::with_shared_machine(
        shared_machine.clone(),
        policy2,
        storage2
    ).with_shared_context(shared_context.clone());

    // エージェント1でエピソードを開始
    agent1.start_episode("エージェント1のタスク", Some(TaskState::Processing)).await?;
    
    // エージェント1が1ステップ実行 (Idle -> Processing)
    let state1 = agent1.step().await?;
    println!("エージェント1の実行結果: {:?}", state1);
    
    // エージェント2でエピソードを開始 (Processing状態から)
    agent2.start_episode("エージェント2のタスク", Some(TaskState::Completed)).await?;
    
    // エージェント2が1ステップ実行 (Processing -> Completed)
    let state2 = agent2.step().await?;
    println!("エージェント2の実行結果: {:?}", state2);
    
    // 共有コンテキストからデータを取得
    let insights: Vec<String> = shared_context.get_keys().unwrap().iter()
        .filter(|k| k.starts_with("insight_"))
        .map(|k| k.to_string())
        .collect();
    println!("共有インサイト数: {}", insights.len());
    
    Ok(())
}
```

## インストール

Cargo.toml に以下を追加:

```toml
[dependencies]
rustate = { version = "0.2.4", features = ["integration"] }
rustate-agent = { version = "0.1.0" }
```

## ライセンス

MIT 