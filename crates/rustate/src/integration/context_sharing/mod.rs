//! # コンテキスト共有パターン
//! 
//! 複数のステートマシン間で共有コンテキストを使用してデータを共有するパターンの実装です。
//! このパターンでは複数のクレートにまたがるステートマシンが同じコンテキストデータに
//! アクセスして読み書きすることができます。
//!
//! ## 概要
//!
//! コンテキスト共有パターンは、複数のステートマシンが同じデータを共有するための柔軟な方法を提供します。
//! このパターンを使用すると、以下のようなメリットがあります：
//!
//! - 複数のステートマシン間でのデータ同期が容易になる
//! - クレート境界をまたいだデータ共有が型安全に行える
//! - イベント転送よりもデータ中心のアプローチでステートマシンを連携させられる
//!
//! ## 主要コンポーネント
//!
//! - `SharedContext`: 複数のステートマシン間で共有できるコンテキストコンテナ
//!
//! ## 使用例
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
//! // マシンA: データを書き込むアクション
//! let write_action = Action::new(
//!     "writeData",
//!     ActionType::Transition,
//!     move |_ctx, _evt| {
//!         let _ = context_for_a.set("status", "active");
//!         let _ = context_for_a.set("timestamp", 12345);
//!     }
//! );
//!
//! // マシンA: 状態マシンを作成
//! let machine_a = MachineBuilder::new("machineA")
//!     .state(State::new("idle"))
//!     .state(State::new("running"))
//!     .initial("idle")
//!     .transition(Transition::new("idle", "START", "running"))
//!     .on_entry("running", write_action)
//!     .build()
//!     .unwrap();
//!
//! // マシンB: データを読み込むアクション
//! let read_action = Action::new(
//!     "readData",
//!     ActionType::Transition,
//!     move |ctx, _evt| {
//!         if let Ok(Some(status)) = context_for_b.get::<String>("status") {
//!             let _ = ctx.set("localStatus", status);
//!         }
//!         if let Ok(Some(timestamp)) = context_for_b.get::<i64>("timestamp") {
//!             let _ = ctx.set("localTimestamp", timestamp);
//!         }
//!     }
//! );
//!
//! // マシンB: 状態マシンを作成
//! let machine_b = MachineBuilder::new("machineB")
//!     .state(State::new("waiting"))
//!     .state(State::new("processing"))
//!     .initial("waiting")
//!     .transition(Transition::new("waiting", "PROCESS", "processing"))
//!     .on_entry("processing", read_action)
//!     .build()
//!     .unwrap();
//!
//! // マシンAを実行 (データを書き込む)
//! machine_a.send("START").unwrap();
//!
//! // マシンBを実行 (データを読み込む)
//! machine_b.send("PROCESS").unwrap();
//! ```
//!
//! ## 実装の詳細
//!
//! このパターンでは、`Arc<RwLock<serde_json::Value>>` を使用してJSON形式のデータを安全に共有します。
//! これにより、複数のステートマシンが同時にコンテキストデータにアクセスしても、データの整合性が
//! 保たれるようになっています。読み込み操作は並行して行えますが、書き込み操作は排他的に実行されます。
//!
//! `SharedContext` はキーと値のペアをJSONオブジェクトとして保存します。
//! この方法により、様々な型のデータを柔軟に格納でき、Serdeを通じた型安全なアクセスが可能になります。
//!
//! ## 制限事項
//!
//! - 大量のデータや高頻度のアクセスが発生する場合、パフォーマンスに影響する可能性があります
//! - 複雑なデータ構造の場合、JSONシリアライズのオーバーヘッドが発生します
//! - 書き込み操作が頻繁に行われる場合、読み込みのブロックが発生する可能性があります

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