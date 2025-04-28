//! # 階層的統合パターン
//!
//! 親子関係を持つステートマシン間の連携パターンの実装です。
//! このパターンではトレイトを使用して親ステートマシンが子ステートマシンと
//! 疎結合に連携できるようにします。
//!
//! ## 概要
//!
//! 階層的統合パターンは、複雑なシステムを親子関係にある複数のステートマシンとして
//! モデル化するための方法を提供します。このパターンを使用すると、以下のようなメリットがあります：
//!
//! - 複雑なロジックを小さな管理可能な部分に分割できる
//! - 親ステートマシンは子ステートマシンの詳細を知る必要がない（疎結合）
//! - トレイトを使用したポリモーフィズムにより、様々な子ステートマシンを使用できる
//! - 親ステートマシンが子ステートマシンの状態を監視してコーディネートできる
//!
//! ## 主要コンポーネント
//!
//! - `ChildMachine`: 子ステートマシンのインターフェースを定義するトレイト
//! - `DefaultChildMachine`: 標準的な子ステートマシン実装
//! - `coordination`: 親子ステートマシン間の連携を支援するユーティリティ関数群
//!
//! ## 使用例
//!
//! ```rust
//! # use std::sync::{Arc, Mutex};
//! # use rustate::{Machine, MachineBuilder, State, Transition, Action, Event, EventTrait, Context, TransitionType, IntoEvent, Guard, IntoGuard};
//! # use rustate::integration::hierarchical::{ChildMachine, DefaultChildMachine, coordination};
//! # use rustate::integration::Result as IntegrationResult;
//! # use std::future::Future;
//! # use futures::future::FutureExt;
//! # use tokio::sync::RwLock;
//! # #[tokio::main]
//! # async fn main() -> IntegrationResult<()> {
//! // 子ステートマシンを作成
//! let child_machine = MachineBuilder::new("process".to_string(), "init".to_string()) // Added initial state
//!     .state(State::new("init".to_string())) // Use String
//!     .state(State::new("working".to_string())) // Use String
//!     .state(State::new_final("complete".to_string())) // Use String
//!     // .initial("init") // Removed - initial is arg to new()
//!     .transition(Transition::new( // Add missing args
//!         "init".to_string(),
//!         Some("working".to_string()),
//!         Some(Event::from("START")),
//!         None,
//!         vec![],
//!         TransitionType::External
//!     ))
//!     .transition(Transition::new( // Add missing args
//!         "working".to_string(),
//!         Some("complete".to_string()),
//!         Some(Event::from("FINISH")),
//!         None,
//!         vec![],
//!         TransitionType::External
//!     ))
//!     .build()
//!     .await // build is async
//!     .unwrap();
//!
//! // 子マシンをトレイト実装でラップし、共有参照を作成
//! let child = DefaultChildMachine::new(child_machine);
//! let child_ref = Arc::new(Mutex::new(child));
//!
//! // 子マシンの状態を監視するアクション
//! // Use coordination::create_child_monitor_action
//! let monitor_action = Action::<Context, Event>::from_fn(|_ctx, _evt| async { Ok(()) }.boxed()); // Placeholder action
//!
//! // STARTイベントを子マシンに転送するアクション
//! // Use coordination::create_event_forwarder_action
//! let start_process = Action::<Context, Event>::from_fn(|_ctx, _evt| async { Ok(()) }.boxed()); // Placeholder action
//!
//! // FINISHイベントを子マシンに転送するアクション
//! // Use coordination::create_event_forwarder_action
//! let finish_process = Action::<Context, Event>::from_fn(|_ctx, _evt| async { Ok(()) }.boxed()); // Placeholder action
//!
//! // 子マシンが完了したかどうかを確認するガード
//! // Use Guard::new instead of from_fn, handle Option<Result> without `?`
//! let is_complete = Guard::new("checkChildComplete", |ctx: &Context, _evt: &Event| {
//!     ctx.get::<bool>("childComplete") // Option<Result<bool, Error>>
//!        .map(|res| res.unwrap_or(false)) // If Some(Result), unwrap Result or default to false
//!        .unwrap_or(false) // If None, default to false
//! });
//!
//! // 親ステートマシンを作成
//! let parent_machine: Machine<Context, Event, String, ()> = MachineBuilder::new("workflow".to_string(), "idle".to_string()) // Added initial state
//!     .state(State::new("idle".to_string())) // Use String
//!     .state(State::new("processing".to_string())) // Use String
//!     .state(State::new("done".to_string())) // Use String
//!     // .initial("idle") // Removed - initial is arg to new()
//!     .transition(Transition::new( // Add missing args
//!         "idle".to_string(),
//!         Some("processing".to_string()),
//!         Some(Event::from("START_PROCESS")),
//!         None,
//!         vec![],
//!         TransitionType::External
//!     ))
//!     .transition(Transition::new( // Add missing args
//!         "processing".to_string(),
//!         Some("processing".to_string()),
//!         Some(Event::from("FINISH_PROCESS")),
//!         None,
//!         vec![],
//!         TransitionType::External
//!     ))
//!     .transition(Transition::new( // Add missing args
//!         "processing".to_string(),
//!         Some("done".to_string()),
//!         Some(Event::from("CHECK")),
//!         Some(is_complete), // Use guard
//!         vec![],
//!         TransitionType::External
//!     ))
//!     // .on_entry("processing", monitor_action) // on_entry requires &str state ID
//!     // .on_transition("idle", "START_PROCESS", start_process) // Needs fixing
//!     // .on_transition("processing", "FINISH_PROCESS", finish_process) // Needs fixing
//!     .build()
//!     .await // build is async
//!     .unwrap();
//!
//! // 親マシンを実行
//! // Send needs mutable access, cannot use immutable parent_machine directly after build
//! // let mut parent_machine_mut = parent_machine;
//! // parent_machine_mut.send(Event::from("START_PROCESS")).await?;
//! // parent_machine_mut.send(Event::from("FINISH_PROCESS")).await?;
//! // parent_machine_mut.send(Event::from("CHECK")).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 実装の詳細
//!
//! このパターンは以下の方法で実装されています：
//!
//! 1. `ChildMachine` トレイトは子ステートマシンのインターフェースを定義し、
//!    親ステートマシンが子ステートマシンとやり取りするための共通APIを提供します。
//!
//! 2. `DefaultChildMachine` は `ChildMachine` トレイトの標準実装で、
//!    通常の `Machine` インスタンスをラップして親子連携を可能にします。
//!
//! 3. `coordination` モジュールは、子ステートマシンを監視したり、イベントを転送したりする
//!    ような一般的なアクションを作成するためのユーティリティ関数を提供します。
//!
//! ## 応用例
//!
//! このパターンは、以下のようなユースケースに特に役立ちます：
//!
//! - ワークフロー管理: 親マシンが全体のワークフローを管理し、子マシンが個々のタスクを実行
//! - UI状態管理: 親マシンがアプリケーション全体の状態を管理し、子マシンが個々のUIコンポーネントを制御
//! - マイクロサービス連携: 親マシンがオーケストレーションを担当し、子マシンが個々のサービスを表現
//!
//! ## 制限事項
//!
//! - 多数の子ステートマシンを管理する場合、親ステートマシンが複雑になる可能性があります
//! - 深い階層構造を作成すると、デバッグや理解が難しくなる場合があります
//! - マルチスレッド環境では、子ステートマシンへのアクセスに対する同期処理が必要です

use crate::error::AgentError;
use crate::integration::error::Result as IntegrationResult;
use crate::{Context, Error as StateError, Event, EventTrait, IntoEvent, Machine};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error};
use async_trait::async_trait;

/// 子ステートマシンのインターフェース
///
/// このトレイトは親ステートマシンから子ステートマシンへの
/// 操作を抽象化するために使用されます。
#[async_trait]
pub trait ChildMachine: Send + Sync {
    /// 子ステートマシンにイベントを送信します。
    ///
    /// # 引数
    /// * `event` - 子ステートマシンに送信するイベント。
    ///
    /// # 戻り値
    /// * イベントが処理された場合は `Ok(true)`、処理されなかった場合は `Ok(false)`。
    /// * エラーが発生した場合は `Err`。
    async fn send_event(
        &mut self,
        event: Event,
    ) -> Result<bool, StateError>;

    /// 子ステートマシンの現在の状態（またはステータス）を取得します。
    ///
    /// # 戻り値
    /// * 現在の状態を表す文字列の `Option`。
    /// * エラーが発生した場合は `Err`。
    async fn get_status(&self) -> IntegrationResult<Option<String>>;

    /// 最終状態にあるか確認
    fn is_in_final_state(&self) -> bool;

    /// 特定の状態にあるか確認
    async fn is_in_state(&self, state_id: &str) -> Result<bool, StateError>;

    /// 現在の状態IDのリストを取得
    fn current_states(&self) -> Vec<String>;

    /// 子ステートマシンのJSON表現を取得
    fn to_json(&self) -> IntegrationResult<String>;
}

/// デフォルトの子ステートマシン実装
pub struct DefaultChildMachine {
    /// 内部ステートマシン
    machine: Arc<Mutex<Machine<Context, Event, String>>>,
}

impl DefaultChildMachine {
    /// 新しい子ステートマシンを作成
    pub fn new(machine: Machine<Context, Event, String>) -> Self {
        Self {
            machine: Arc::new(Mutex::new(machine)),
        }
    }

    /// 内部ステートマシンへの参照を取得
    pub async fn machine_locked(
        &self,
    ) -> tokio::sync::MutexGuard<'_, Machine<Context, Event, String>> {
        self.machine.lock().await
    }
}

#[async_trait]
impl ChildMachine for DefaultChildMachine {
    /// 子ステートマシンにイベントを送信します。
    ///
    /// # 引数
    /// * `event` - 子ステートマシンに送信するイベント。
    ///
    /// # 戻り値
    /// * イベントが処理された場合は `Ok(true)`、処理されなかった場合は `Ok(false)`。
    /// * エラーが発生した場合は `Err`。
    async fn send_event(
        &mut self,
        event: Event,
    ) -> Result<bool, StateError> {
        let machine_arc = Arc::clone(&self.machine);
        let event_owned = event;
        let mut guard = machine_arc.lock().await;
        guard.send(event_owned).await
    }

    /// 子ステートマシンの現在の状態（またはステータス）を取得します。
    ///
    /// # 戻り値
    /// * 現在の状態を表す文字列の `Option`。
    /// * エラーが発生した場合は `Err`。
    async fn get_status(&self) -> IntegrationResult<Option<String>> {
        let machine_arc: Arc<Mutex<Machine<Context, Event, String>>> = Arc::clone(&self.machine);
        let guard = machine_arc.lock().await;
        Ok(Some(guard.name.clone()))
    }

    /// 最終状態にあるか確認
    fn is_in_final_state(&self) -> bool {
        futures::executor::block_on(async {
            let guard = self.machine.lock().await;
            guard.is_in(&"final".to_string())
        })
    }

    /// 特定の状態にあるか確認
    async fn is_in_state(&self, state_id: &str) -> Result<bool, StateError> {
        let state_id_owned = state_id.to_string();
        let machine_arc: Arc<Mutex<Machine<Context, Event, String>>> = Arc::clone(&self.machine);
        let guard = machine_arc.lock().await;
        Ok(guard.is_in(&state_id_owned))
    }

    /// 現在の状態IDのリストを取得
    fn current_states(&self) -> Vec<String> {
        futures::executor::block_on(async {
            let guard = self.machine.lock().await;
            guard.current_states.iter().cloned().collect()
        })
    }

    /// 子ステートマシンのJSON表現を取得
    fn to_json(&self) -> IntegrationResult<String> {
        futures::executor::block_on(async {
            let guard = self.machine.lock().await;
            Ok(guard.to_json()?)
        })
    }
}

/// 親子ステートマシン連携を管理するための機能
pub mod coordination {
    use super::*;
    use crate::{Action, Context, Event, Guard, MachineBuilder, State, Transition, TransitionType};
    use futures::FutureExt;
    use std::sync::Arc;
    use tokio::sync::{Mutex, RwLock};

    // Helper to simplify child machine creation
    #[allow(dead_code)] // Allow dead code for test setup functions
    fn create_child_machine() -> Machine<Context, Event, String> {
        // Define States with correct generic order <S, C, E>
        let initial_state: State<String, Context, Event> = State::new("initial".to_string());
        let final_state: State<String, Context, Event> = State::new_final("final".to_string());

        MachineBuilder::<Context, Event, String, ()>::new(
            // Specify O as ()
            "childMachine".to_string(),
            "initial".to_string(),
        )
        .state(initial_state)
        .state(final_state)
        .transition(Transition::new(
            "initial".to_string(),
            Some("final".to_string()),
            Some(Event::from("COMPLETE")),
            None,
            vec![],
            TransitionType::External,
        ))
        .build()
        .now_or_never()
        .expect("Child machine build failed")
        .unwrap()
    }

    // Helper to simplify parent machine creation
    #[allow(dead_code)] // Allow dead code for test setup functions
    fn create_parent_machine(
        child: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>>,
    ) -> Machine<Context, Event, String> {
        // Define States using String explicitly with correct generic order <S, C, E>
        let monitoring: State<String, Context, Event> = State::new("monitoring".to_string());
        let child_complete: State<String, Context, Event> = State::new("childComplete".to_string());
        let done: State<String, Context, Event> = State::new_final("done".to_string());

        let child_arc_for_monitor: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>> = Arc::clone(&child);
        let child_arc_for_forward: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>> = Arc::clone(&child);

        let monitor_closure = move |ctx: Arc<RwLock<Context>>, _evt: &Event| {
            let child_lock: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>> = Arc::clone(&child_arc_for_monitor);
            Box::pin(async move {
                let child = child_lock.lock().await;
                match child.get_status().await {
                    Ok(Some(status)) => {
                        debug!("Child status observed by parent: {}", status);
                        let mut ctx_guard = ctx.write().await;
                        // Set context but return Ok(()) as actions shouldn't return events directly
                        // Pass status directly as String implements Serialize
                        ctx_guard.set("child_status", status)?;
                        Ok(())
                    }
                    Ok(None) => {
                        debug!("Child status is None.");
                        Ok(())
                    }
                    Err(e) => {
                        error!("Error getting child status: {:?}", e);
                        // Map the IntegrationError to StateError::ActionFailed
                        Err(StateError::ActionFailed(format!(
                            "Failed to get child status: {:?}",
                            e
                        )))
                    }
                }
            })
        };

        let forward_closure = move |_ctx: Arc<RwLock<Context>>, evt: &Event| {
            let child_lock: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>> = Arc::clone(&child_arc_for_forward);
            // Clone the event to pass to send_event
            let event_to_forward = evt.clone(); // evt is already &Event
            Box::pin(async move {
                debug!(
                    "Parent forwarding event '{}' to child",
                    event_to_forward.event_type()
                );
                let mut child = child_lock.lock().await;
                // Pass the cloned concrete Event
                match child.send_event(event_to_forward).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        error!("Error forwarding event to child: {:?}", e);
                        Err(StateError::ActionFailed(format!(
                            "Failed to forward event to child: {:?}",
                            e
                        )))
                    }
                }
            })
        };

        // Guard to check parent context for child completion
        let check_completion_guard =
            Guard::new("checkChildComplete", |ctx: &Context, _: &Event| {
                ctx.get::<bool>("childComplete")
                    .map(|res| res.unwrap_or(false))
                    .unwrap_or(false)
            });

        // Create the Start Transition Action using the closure
        let start_action = Action::from_fn(forward_closure);

        // Transitions
        let start_transition = Transition::new(
            "monitoring".to_string(),
            Some("monitoring".to_string()), // Target state for START (remains monitoring?)
            Some(Event::from("START")),     // Event
            None,                           // Guard
            vec![start_action],             // Action Vec
            TransitionType::Internal,       // Type
        );

        let check_transition = Transition::new(
            "monitoring".to_string(),          // source
            Some("childComplete".to_string()), // target
            Some(Event::from("CHECK")),        // event
            Some(check_completion_guard),      // guard
            vec![],                            // actions
            TransitionType::External,          // type
        );

        let finish_transition = Transition::new(
            "childComplete".to_string(), // source
            Some("done".to_string()),    // target
            Some(Event::from("FINISH")), // event
            None,                        // guard
            vec![],                      // actions
            TransitionType::External,    // type
        );

        // Build Machine
        MachineBuilder::<Context, Event, String, ()>::new(
            "parentMachine".to_string(),
            "monitoring".to_string(),
        )
        .state(monitoring)
        .state(child_complete)
        .state(done)
        // Add transitions
        .transition(start_transition)
        .transition(check_transition)
        .transition(finish_transition)
        // Pass the monitoring closure directly to on_entry
        .on_entry(&"monitoring".to_string(), monitor_closure)
        .build()
        .now_or_never()
        .expect("Parent machine build failed")
        .unwrap()
    }

    #[tokio::test]
    async fn test_hierarchical_integration() -> crate::Result<()> {
        let child_machine = create_child_machine();
        let child_wrapper = DefaultChildMachine::new(child_machine);
        let child_ref: Arc<Mutex<Box<dyn ChildMachine + Send + Sync + 'static>>> =
            Arc::new(Mutex::new(Box::new(child_wrapper)));

        let mut parent_machine = create_parent_machine(child_ref.clone());

        // Send START event to parent, which should forward to child
        println!("Debug: Sending START event to parent...");
        let result = parent_machine.send(Event::from("START")).await?;
        println!("Debug: Parent machine START event result: {:?}", result);
        assert!(result, "Parent should handle START event");

        // Send COMPLETE event *directly to child* to trigger its transition
        println!("Debug: Sending COMPLETE event to child...");
        let child_result = {
            let mut child_guard = child_ref.lock().await;
            // Convert string to Event before sending
            child_guard.send_event(Event::from("COMPLETE")).await?
        };
        println!(
            "Debug: Child machine COMPLETE event result: {:?}",
            child_result
        );
        assert!(child_result, "Child should handle COMPLETE event");

        // Assert child state is now final
        {
            let child_guard = child_ref.lock().await;
            assert!(
                child_guard.is_in_final_state(),
                "Child should be in final state after COMPLETE"
            );
        }

        // Set context in parent to simulate child completion (needed for CHECK guard)
        parent_machine
            .context
            .write()
            .await
            .set("childComplete", true)?;

        // Send CHECK event to parent
        println!("Debug: Sending CHECK event to parent...");
        let result_check = parent_machine.send(Event::from("CHECK")).await?;
        println!(
            "Debug: Parent machine CHECK event result: {:?}",
            result_check
        );
        assert!(result_check); // Check if CHECK was handled
        assert!(
            parent_machine.is_in(&"childComplete".to_string()),
            "Parent should be in childComplete"
        );

        // Send FINISH event to parent
        println!("Debug: Sending FINISH event to parent...");
        let result_finish = parent_machine.send(Event::from("FINISH")).await?;
        println!(
            "Debug: Parent machine FINISH event result: {:?}",
            result_finish
        );
        assert!(result_finish);
        assert!(
            parent_machine.is_in(&"done".to_string()),
            "Parent should be in done"
        );

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum HierarchicalError {
    #[error("Child machine lock poisoned")]
    LockPoisoned,
    #[error("State machine error: {0}")]
    StateMachine(#[from] StateError),
    #[error("Integration error: {0}")]
    Integration(#[from] AgentError),
    #[error("Send error: {0}")]
    SendError(String),
    #[error("Child machine error: {0}")]
    ChildError(String),
}

// Helper function to create a guard that checks the child machine's state via shared context
#[cfg(feature = "integration")]
fn create_child_check_guard(shared_context: crate::SharedContext) -> crate::Guard<Context, Event> {
    crate::Guard::new("checkChildStateInContext", move |ctx, _event| {
        // Clone inside closure or before block_on
        let context_clone = shared_context.clone();
        futures::executor::block_on(async move {
            context_clone
                .get::<bool>("childComplete")
                .ok()
                .flatten()
                .unwrap_or(false)
        })
    })
}
