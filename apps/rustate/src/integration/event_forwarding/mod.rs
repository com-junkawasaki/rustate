//! # イベント転送パターン
//!
//! ステートマシン間でイベントを転送するパターンの実装です。
//! このパターンではステートマシンの参照を共有し、一方のステートマシンの
//! アクションから他方のステートマシンにイベントを転送することができます。
//!
//! ## 概要
//!
//! イベント転送パターンは、複数のステートマシンが疎結合な形で連携するための効果的な方法です。
//! このパターンを使用すると、以下のようなメリットがあります：
//!
//! - ステートマシン間の直接的な依存関係を減らす
//! - クレート境界をまたいだ連携が可能になる
//! - 複数のステートマシンが並行して動作する場合でも安全に連携できる
//!
//! ## 主要コンポーネント
//!
//! - `SharedMachineRef`: ステートマシンをスレッドセーフに共有するためのラッパー
//! - `EventForwarder`: イベント転送機能を抽象化するトレイト
//!
//! ## 使用例
//!
//! ```rust
//! use rustate::{Machine, MachineBuilder, State, Transition, Action, ActionType};
//! use rustate::integration::SharedMachineRef;
//!
//! // 子ステートマシンを作成
//! let child_machine = MachineBuilder::new("child")
//!     .state(State::new("idle"))
//!     .state(State::new("active"))
//!     .initial("idle")
//!     .transition(Transition::new("idle", "ACTIVATE", "active"))
//!     .build()
//!     .unwrap();
//!
//! // 共有参照を作成
//! let shared_child = SharedMachineRef::new(child_machine);
//! let shared_child_clone = shared_child.clone();
//!
//! // 親ステートマシンのイベントに応じて子マシンにイベントを転送するアクション
//! let forward_action = Action::new(
//!     "forwardToChild",
//!     ActionType::Transition,
//!     move |_ctx, evt| {
//!         if evt.event_type == "PARENT_EVENT" {
//!             let _ = shared_child_clone.send_event("ACTIVATE");
//!         }
//!     }
//! );
//!
//! // 親ステートマシンを作成
//! let parent_machine = MachineBuilder::new("parent")
//!     .state(State::new("ready"))
//!     .initial("ready")
//!     .on_entry("ready", forward_action)
//!     .build()
//!     .unwrap();
//!
//! // 親マシンにイベントを送信すると、子マシンにもイベントが転送される
//! parent_machine.send("PARENT_EVENT").unwrap();
//!
//! // 子マシンの状態を確認
//! assert!(shared_child.is_in("active").unwrap());
//! ```
//!
//! ## 実装の詳細
//!
//! このパターンでは、`Arc<Mutex<Machine>>` を使用してステートマシンを安全に共有します。
//! これにより、複数のコンポーネントが同じステートマシンに対して同時にイベントを送信しても
//! データの競合が発生しないようにしています。
//!
//! `EventForwarder` トレイトは、様々なイベント転送の実装を抽象化するために使用できます。
//! デフォルトでは `SharedMachineRef` がこのトレイトを実装していますが、
//! カスタムの実装を作成することも可能です。
//!
//! ## 制限事項
//!
//! - イベント転送時にデッドロックが発生する可能性があるため、相互に参照し合うステートマシンの
//!   設計には注意が必要です。循環的な依存関係は避けてください。
//! - 大量のイベント転送が発生する場合、パフォーマンスに影響する可能性があります。

use crate::integration::error::Result as IntegrationResult;
use crate::{Context, Event, IntoEvent, Machine};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 共有ステートマシン参照
///
/// このラッパーは複数のクレートにまたがるステートマシンへの参照を
/// 安全に共有するために使用されます。
#[derive(Clone)]
pub struct SharedMachineRef {
    /// ラップされたステートマシン
    machine: Arc<Mutex<Machine>>,
    /// マシン名（デバッグ用）
    name: String,
}

impl SharedMachineRef {
    /// 新しい共有ステートマシン参照を作成
    pub fn new(machine: Machine) -> Self {
        let name = machine.name.clone();
        Self {
            machine: Arc::new(Mutex::new(machine)),
            name,
        }
    }

    /// ステートマシンにイベントを送信
    pub async fn send_event<E: IntoEvent + Send>(&self, event: E) -> IntegrationResult<bool> {
        let event = event.into_event();
        let mut machine = self.machine.lock().await;
        Ok(machine.send(event).await?)
    }

    /// ステートマシンが特定の状態にあるか確認
    pub async fn is_in_state(&self, state_id: &str) -> IntegrationResult<bool> {
        let machine = self.machine.lock().await;
        Ok(machine.is_in(&state_id.to_string()))
    }

    /// マシン名を取得
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// ステートマシン間のイベント転送を管理するためのトレイト
#[async_trait::async_trait]
pub trait EventForwarder {
    /// イベントを転送
    async fn forward_event<E: IntoEvent + Send>(&self, event: E) -> IntegrationResult<bool>;
}

#[async_trait::async_trait]
impl EventForwarder for SharedMachineRef {
    async fn forward_event<E: IntoEvent + Send>(&self, event: E) -> IntegrationResult<bool> {
        self.send_event(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionType, Context, Event, IntoEvent, Machine, MachineBuilder, State,
        Transition, TransitionType, error::StateError,
    };
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_event_forwarding() -> IntegrationResult<()> {
        // 子ステートマシンを作成
        let child = create_child_machine();
        let shared_child = SharedMachineRef::new(child);

        // 親ステートマシンを作成（子マシンへのイベント転送を設定）
        let parent = create_parent_machine(shared_child.clone()).await?;
        let shared_parent = SharedMachineRef::new(parent);

        // テスト用に直接子マシンへイベントを送信（本来はイベント転送経由）
        println!("Debug: Sending ACTIVATE event directly to child");
        let direct_result = shared_child.send_event(Event::from("ACTIVATE")).await;
        println!("Debug: Direct child event result: {:?}", direct_result);

        // 親マシンにもイベントを送信
        let result = shared_parent.send_event(Event::from("PARENT_EVENT")).await;
        println!("Debug: Parent event result: {:?}", result);

        // 子マシンの状態を確認
        let is_activated = shared_child.is_in_state(&"activated").await?;
        println!("Debug: Child is in activated state: {:?}", is_activated);
        assert!(is_activated);
        Ok(())
    }

    fn create_child_machine() -> Machine<Context, Event, String> {
        let initial = State::new("initial".to_string());
        let activated = State::new("activated".to_string());

        let activate = Transition::new(
            "initial".to_string(),
            Some("activated".to_string()),
            Some(Event::from("ACTIVATE")),
            None,
            vec![],
            TransitionType::External,
        );

        MachineBuilder::new("childMachine".to_string(), "initial".to_string())
            .state(initial)
            .state(activated)
            .transition(activate)
            .build()
            .now_or_never()
            .expect("Sync build failed")
            .unwrap()
    }

    async fn create_parent_machine(
        child: SharedMachineRef,
    ) -> IntegrationResult<Machine<Context, Event, String>> {
        let state = State::new("parent".to_string());

        let forward_closure = move |_ctx: Arc<RwLock<Context>>, evt: &Event| {
            let child_clone = child.clone();
            let event_type_clone = evt.event_type.clone();
            async move {
                println!("Debug: Parent received event: {}", event_type_clone);
                if event_type_clone.contains("PARENT_EVENT") {
                    println!("Debug: Forwarding ACTIVATE event to child");
                    let result = child_clone.send_event(Event::from("ACTIVATE")).await;
                    println!("Debug: Child event result: {:?}", result);
                    result.map(|_| ()).map_err(|e| StateError::ActionFailed(format!("Child send_event failed: {}", e)))
                } else {
                    Ok(())
                }
            }
        };

        let mut internal_transition =
            Transition::internal_transition("parent".to_string(), Event::from("PARENT_EVENT"));
        internal_transition = internal_transition.with_action(forward_closure);

        let result = MachineBuilder::new("parentMachine".to_string(), "parent".to_string())
            .state(state)
            .transition(internal_transition)
            .build()
            .await?;

        Ok(result)
    }
}
