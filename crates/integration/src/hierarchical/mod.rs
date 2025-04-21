//! # 階層的統合パターン
//! 
//! 親子関係を持つステートマシン間の連携パターンの実装です。
//! このパターンではトレイトを使用して親ステートマシンが子ステートマシンと
//! 疎結合に連携できるようにします。

use rustate::{Machine, IntoEvent};
use crate::error::Result;

/// 子ステートマシンのインターフェース
/// 
/// このトレイトは親ステートマシンから子ステートマシンへの
/// 操作を抽象化するために使用されます。
pub trait ChildMachine: Send + Sync {
    /// 親からのイベントを処理
    fn handle_parent_event<E: IntoEvent>(&mut self, event: E) -> Result<bool>;
    
    /// 最終状態にあるか確認
    fn is_in_final_state(&self) -> bool;
    
    /// 特定の状態にあるか確認
    fn is_in(&self, state_id: &str) -> bool;
    
    /// 現在の状態IDのリストを取得
    fn current_states(&self) -> Vec<String>;
    
    /// 子ステートマシンのJSON表現を取得
    fn to_json(&self) -> Result<String>;
}

/// デフォルトの子ステートマシン実装
pub struct DefaultChildMachine {
    /// 内部ステートマシン
    machine: Machine,
    /// 最終状態ID
    final_state_id: String,
}

impl DefaultChildMachine {
    /// 新しい子ステートマシンを作成
    pub fn new(machine: Machine, final_state_id: impl Into<String>) -> Self {
        Self {
            machine,
            final_state_id: final_state_id.into(),
        }
    }
    
    /// 内部ステートマシンへの参照を取得
    pub fn machine(&self) -> &Machine {
        &self.machine
    }
    
    /// 内部ステートマシンへの可変参照を取得
    pub fn machine_mut(&mut self) -> &mut Machine {
        &mut self.machine
    }
}

impl ChildMachine for DefaultChildMachine {
    fn handle_parent_event<E: IntoEvent>(&mut self, event: E) -> Result<bool> {
        Ok(self.machine.send(event)?)
    }
    
    fn is_in_final_state(&self) -> bool {
        self.machine.is_in(&self.final_state_id)
    }
    
    fn is_in(&self, state_id: &str) -> bool {
        self.machine.is_in(state_id)
    }
    
    fn current_states(&self) -> Vec<String> {
        self.machine.current_states.iter().cloned().collect()
    }
    
    fn to_json(&self) -> Result<String> {
        Ok(self.machine.to_json()?)
    }
}

/// 親子ステートマシン連携を管理するための機能
pub mod coordination {
    use super::*;
    use rustate::{Action, ActionType, Context, Event};
    use std::sync::{Arc, Mutex};
    
    /// 子ステートマシンの状態を監視するアクションを作成
    pub fn create_child_monitor_action<C>(
        name: impl Into<String>,
        child: Arc<Mutex<C>>,
    ) -> Action
    where
        C: ChildMachine + 'static,
    {
        Action::new(
            name,
            ActionType::Transition,
            move |ctx: &mut Context, _evt: &Event| {
                if let Ok(child) = child.lock() {
                    if child.is_in_final_state() {
                        let _ = ctx.set("childComplete", true);
                    }
                    
                    // 子マシンの現在の状態をコンテキストに保存
                    let _ = ctx.set("childStates", child.current_states());
                }
            },
        )
    }
    
    /// 子ステートマシンにイベントを転送するアクションを作成
    pub fn create_event_forwarder_action<C, E>(
        name: impl Into<String>,
        child: Arc<Mutex<C>>,
        parent_event: impl Into<String>,
        child_event: E,
    ) -> Action
    where
        C: ChildMachine + 'static,
        E: IntoEvent + Clone + Send + Sync + 'static,
    {
        let parent_event = parent_event.into();
        Action::new(
            name,
            ActionType::Transition,
            move |_ctx: &mut Context, evt: &Event| {
                if evt.event_type == parent_event {
                    if let Ok(mut child) = child.lock() {
                        let _ = child.handle_parent_event(child_event.clone());
                    }
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustate::{State, Transition, MachineBuilder, Action, ActionType};
    use std::sync::{Arc, Mutex};
    
    #[test]
    fn test_hierarchical_integration() {
        // 子ステートマシンを作成
        let child_machine = create_child_machine();
        let child = DefaultChildMachine::new(child_machine, "final");
        let child = Arc::new(Mutex::new(child));
        
        // 親ステートマシンを作成
        let parent_machine = create_parent_machine(child.clone());
        
        // 親マシンに"START"イベントを送信
        parent_machine.send("START").unwrap();
        
        // 子マシンの状態を確認
        let child = child.lock().unwrap();
        assert!(child.is_in("progress"));
        
        // 子マシンが最終状態にないことを確認
        assert!(!child.is_in_final_state());
    }
    
    fn create_child_machine() -> Machine {
        let initial = State::new("initial");
        let progress = State::new("progress");
        let final_state = State::new_final("final");
        
        let start = Transition::new("initial", "START", "progress");
        let complete = Transition::new("progress", "COMPLETE", "final");
        
        MachineBuilder::new("childMachine")
            .state(initial)
            .state(progress)
            .state(final_state)
            .initial("initial")
            .transition(start)
            .transition(complete)
            .build()
            .unwrap()
    }
    
    fn create_parent_machine(child: Arc<Mutex<impl ChildMachine + 'static>>) -> Machine {
        let monitoring = State::new("monitoring");
        let completed = State::new("completed");
        
        let start_monitoring = Transition::new("monitoring", "CHECK", "completed");
        
        // 子マシンの状態を監視するアクション
        let monitor_action = coordination::create_child_monitor_action(
            "monitorChild",
            child.clone(),
        );
        
        // イベントを転送するアクション
        let forward_action = coordination::create_event_forwarder_action(
            "forwardToChild",
            child,
            "START",
            "START",
        );
        
        // 監視完了を確認するガード
        let check_complete = ("isChildComplete", |ctx: &rustate::Context, _: &rustate::Event| {
            ctx.get::<bool>("childComplete").unwrap_or(false)
        });
        
        MachineBuilder::new("parentMachine")
            .state(monitoring)
            .state(completed)
            .initial("monitoring")
            .on_entry("monitoring", monitor_action)
            .on_entry("monitoring", forward_action)
            .transition(Transition::new("monitoring", "CHECK", "completed").with_guard(check_complete))
            .build()
            .unwrap()
    }
} 