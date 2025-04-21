//! # イベント転送パターン
//! 
//! ステートマシン間でイベントを転送するパターンの実装です。
//! このパターンではステートマシンの参照を共有し、一方のステートマシンの
//! アクションから他方のステートマシンにイベントを転送することができます。

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