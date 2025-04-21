//! # コンテキスト共有パターン
//! 
//! 複数のステートマシン間で共有コンテキストを使用してデータを共有するパターンの実装です。
//! このパターンでは複数のクレートにまたがるステートマシンが同じコンテキストデータに
//! アクセスして読み書きすることができます。

use std::sync::{Arc, RwLock};
use serde::{Serialize, de::DeserializeOwned};
use crate::integration::error::{Result, LockResultExt};

/// 共有コンテキスト
/// 
/// このラッパーは複数のクレートにまたがるステートマシン間で
/// コンテキストデータを安全に共有するために使用されます。
#[derive(Clone, Default)]
pub struct SharedContext {
    /// 共有データ
    data: Arc<RwLock<serde_json::Value>>,
}

impl SharedContext {
    /// 新しい共有コンテキストを作成
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(serde_json::json!({}))),
        }
    }
    
    /// 共有コンテキストから値を取得
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let data = self.data.read().lock_err()?;
        match &*data {
            serde_json::Value::Object(map) => {
                if let Some(val) = map.get(key) {
                    Ok(serde_json::from_value(val.clone())?)
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
    
    /// 共有コンテキストに値を設定
    pub fn set<T: Serialize>(&self, key: &str, value: T) -> Result<()> {
        let mut data = self.data.write().lock_err()?;
        match &mut *data {
            serde_json::Value::Object(map) => {
                map.insert(key.to_string(), serde_json::to_value(value)?);
                Ok(())
            }
            _ => {
                *data = serde_json::json!({ key: value });
                Ok(())
            }
        }
    }
    
    /// キーが存在するか確認
    pub fn contains_key(&self, key: &str) -> Result<bool> {
        let data = self.data.read().lock_err()?;
        match &*data {
            serde_json::Value::Object(map) => Ok(map.contains_key(key)),
            _ => Ok(false),
        }
    }
    
    /// 共有コンテキストからキーを削除
    pub fn remove(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let mut data = self.data.write().lock_err()?;
        match &mut *data {
            serde_json::Value::Object(map) => Ok(map.remove(key)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Machine, MachineBuilder, State, Action, ActionType, Transition};
    
    #[test]
    fn test_context_sharing() {
        // 共有コンテキストを作成
        let shared_context = SharedContext::new();
        
        // 2つのステートマシンを作成（両方とも同じ共有コンテキストを使用）
        let (machine_a, machine_b) = create_machines(shared_context.clone());
        
        // マシンAを"updateState"イベントで実行
        machine_a.send("UPDATE_STATE").unwrap();
        
        // マシンBを"readState"イベントで実行
        machine_b.send("READ_STATE").unwrap();
        
        // 共有コンテキストの値を確認
        let status = shared_context.get::<String>("status").unwrap();
        assert_eq!(status, Some("active".to_string()));
        
        let counter = shared_context.get::<i32>("counter").unwrap();
        assert_eq!(counter, Some(1));
    }
    
    fn create_machines(shared_context: SharedContext) -> (Machine, Machine) {
        // クローンを作成して別々のクロージャに渡す
        let context_for_a = shared_context.clone();
        let context_for_b = shared_context;
        
        // マシンA: 状態を更新する
        let state_a = State::new("stateA");
        let update_action = Action::new(
            "updateStatus",
            ActionType::Transition,
            move |_ctx, _evt| {
                let _ = context_for_a.set("status", "active");
                let counter = context_for_a.get::<i32>("counter").unwrap().unwrap_or(0);
                let _ = context_for_a.set("counter", counter + 1);
            },
        );
        
        let machine_a = MachineBuilder::new("machineA")
            .state(state_a)
            .initial("stateA")
            .on_entry("stateA", update_action)
            .transition(Transition::internal_transition("stateA", "UPDATE_STATE"))
            .build()
            .unwrap();
            
        // マシンB: 状態を読み取る
        let state_b = State::new("stateB");
        let read_action = Action::new(
            "readStatus",
            ActionType::Transition,
            move |ctx, _evt| {
                if let Ok(Some(status)) = context_for_b.get::<String>("status") {
                    let _ = ctx.set("localStatus", status);
                }
                
                if let Ok(Some(counter)) = context_for_b.get::<i32>("counter") {
                    let _ = ctx.set("localCounter", counter);
                }
            },
        );
        
        let machine_b = MachineBuilder::new("machineB")
            .state(state_b)
            .initial("stateB")
            .on_entry("stateB", read_action)
            .transition(Transition::internal_transition("stateB", "READ_STATE"))
            .build()
            .unwrap();
            
        (machine_a, machine_b)
    }
} 