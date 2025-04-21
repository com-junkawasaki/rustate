//! # RuState Integration
//! 
//! このモジュールはRuStateステートマシンをクレート間で型安全に統合するためのパターンを提供します。
//! 
//! ## 主な統合パターン
//! 
//! 1. **イベント転送パターン**: 複数のステートマシン間でイベントを転送し、疎結合な連携を実現します。
//!    ステートマシンの参照を共有し、一方のマシンのアクションから他方のマシンにイベントを送信できます。
//! 
//! 2. **コンテキスト共有パターン**: 複数のステートマシン間で共有コンテキストを使用してデータを連携します。
//!    これにより異なるクレートにまたがるステートマシンが同じデータにアクセスし、状態を同期できます。
//! 
//! 3. **階層的統合パターン**: 親子関係を持つステートマシン間の連携を実現します。トレイトを使用して
//!    親ステートマシンが子ステートマシンと疎結合に連携できるようにします。
//!
//! ## 使用例
//!
//! ### イベント転送パターン
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
//! // 親ステートマシンのイベントに応じて子マシンにイベントを転送
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
//! ```
//!
//! ### コンテキスト共有パターン
//!
//! ```rust
//! use rustate::{Machine, MachineBuilder, State, Transition, Action, ActionType};
//! use rustate::integration::SharedContext;
//!
//! // 共有コンテキストを作成
//! let shared_context = SharedContext::new();
//! let context_for_a = shared_context.clone();
//! let context_for_b = shared_context.clone();
//!
//! // データを書き込むアクション
//! let write_action = Action::new(
//!     "writeData",
//!     ActionType::Transition,
//!     move |_ctx, _evt| {
//!         let _ = context_for_a.set("status", "active");
//!     }
//! );
//!
//! // データを読み込むアクション
//! let read_action = Action::new(
//!     "readData",
//!     ActionType::Transition,
//!     move |ctx, _evt| {
//!         if let Ok(Some(status)) = context_for_b.get::<String>("status") {
//!             let _ = ctx.set("localStatus", status);
//!         }
//!     }
//! );
//! ```
//!
//! ### 階層的統合パターン
//!
//! ```rust
//! use std::sync::{Arc, Mutex};
//! use rustate::{Machine, MachineBuilder, State, Transition};
//! use rustate::integration::{ChildMachine, DefaultChildMachine};
//! use rustate::integration::hierarchical::coordination;
//!
//! // 子ステートマシンを作成
//! let child_machine = MachineBuilder::new("child")
//!     .state(State::new("initial"))
//!     .state(State::new("running"))
//!     .state(State::new_final("complete"))
//!     .initial("initial")
//!     .transition(Transition::new("initial", "START", "running"))
//!     .transition(Transition::new("running", "COMPLETE", "complete"))
//!     .build()
//!     .unwrap();
//!
//! // 子マシンをトレイト実装で包む
//! let child = DefaultChildMachine::new(child_machine, "complete");
//! let child = Arc::new(Mutex::new(child));
//!
//! // 子マシンを監視するアクション
//! let monitor_action = coordination::create_child_monitor_action(
//!     "monitorChild",
//!     child.clone()
//! );
//!
//! // 子マシンにイベントを転送するアクション
//! let forward_action = coordination::create_event_forwarder_action(
//!     "forwardToChild",
//!     child,
//!     "PARENT_START",
//!     "START"
//! );
//! ```

pub mod event_forwarding;
pub mod context_sharing;
pub mod hierarchical;

/// エラー型
pub mod error;
pub use error::{Error, Result, LockResultExt};

/// 再エクスポートして便利なインターフェースを提供
pub use event_forwarding::SharedMachineRef;
pub use context_sharing::SharedContext;
pub use hierarchical::ChildMachine; 