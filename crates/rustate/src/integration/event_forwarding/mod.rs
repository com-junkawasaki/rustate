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

use std::sync::{Arc, Mutex};
use crate::{IntoEvent, Machine};
use crate::integration::error::{Result, LockResultExt};

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
    pub fn send_event<E: IntoEvent>(&self, event: E) -> Result<bool> {
        let mut machine = self.machine.lock().lock_err()?;
        Ok(machine.send(event)?)
    }
    
    /// ステートマシンが特定の状態にあるか確認
    pub fn is_in(&self, state_id: &str) -> Result<bool> {
        let machine = self.machine.lock().lock_err()?;
        Ok(machine.is_in(state_id))
    }
    
    /// マシン名を取得
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// ステートマシン間のイベント転送を管理するためのトレイト
pub trait EventForwarder {
    /// イベントを転送
    fn forward_event<E: IntoEvent>(&self, event: E) -> Result<bool>;
}

impl EventForwarder for SharedMachineRef {
    fn forward_event<E: IntoEvent>(&self, event: E) -> Result<bool> {
        self.send_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, ActionType, MachineBuilder, State, Transition};
    
    #[test]
    fn test_event_forwarding() {
        // 子ステートマシンを作成
        let child = create_child_machine();
        let shared_child = SharedMachineRef::new(child);
        
        // 親ステートマシンを作成（子マシンへのイベント転送を設定）
        let parent = create_parent_machine(shared_child.clone());
        let shared_parent = SharedMachineRef::new(parent);
        
        // 親マシンにイベントを送信
        let result = shared_parent.send_event("PARENT_EVENT");
        assert!(result.is_ok());
        
        // 子マシンの状態を確認
        assert!(shared_child.is_in("activated").unwrap());
    }
    
    fn create_child_machine() -> Machine {
        let initial = State::new("initial");
        let activated = State::new("activated");
        
        let activate = Transition::new("initial", "ACTIVATE", "activated");
        
        MachineBuilder::new("childMachine")
            .state(initial)
            .state(activated)
            .initial("initial")
            .transition(activate)
            .build()
            .unwrap()
    }
    
    fn create_parent_machine(child: SharedMachineRef) -> Machine {
        let state = State::new("parent");
        
        // 子マシンにイベントを転送するアクション
        let forward_to_child = Action::new(
            "forwardToChild",
            ActionType::Entry,
            move |_ctx, evt| {
                if evt.event_type == "PARENT_EVENT" {
                    let _ = child.send_event("ACTIVATE");
                }
            },
        );
        
        MachineBuilder::new("parentMachine")
            .state(state)
            .initial("parent")
            .on_entry("parent", forward_to_child)
            .build()
            .unwrap()
    }
} 