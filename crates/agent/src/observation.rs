use crate::error::AgentError;
use rustate::{StateTrait, EventTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};

/// 観測データは、状態遷移に関する情報を記録します。
/// 前の状態、イベント、結果の状態、メタデータなどを含みます。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation<S, E>
where
    S: StateTrait,
    E: EventTrait,
{
    /// 観測の一意な識別子
    pub id: String,

    /// 観測のタイムスタンプ (UNIXエポックからの秒数)
    pub timestamp: u64,

    /// 遷移前の状態
    pub previous_state: S,

    /// 遷移を引き起こしたイベント
    pub event: E,

    /// 遷移後の状態
    pub next_state: S,

    /// この観測に関連する追加のメタデータ
    pub metadata: HashMap<String, String>,
}

/// 状態遷移の観測データに関するメソッド
impl<S, E> Observation<S, E>
where
    S: StateTrait,
    E: EventTrait,
{
    /// 新しい観測を作成します
    pub fn new(previous_state: S, event: E, next_state: S) -> Self {
        Self {
            id: format!("obs-{}", uuid::Uuid::new_v4()),
            previous_state,
            event,
            next_state,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("時間が取得できませんでした")
                .as_secs(),
            metadata: HashMap::new(),
        }
    }
    
    /// メタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// 複数のメタデータを一度に追加します
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }
}

/// 現在のUNIXタイムスタンプを返します
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// 一意なIDを生成します
fn uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp();
    format!("obs-{}-{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustate::{EventTrait, StateTrait};

    #[derive(Debug, Clone, PartialEq)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl StateTrait for TestState {
        fn id(&self) -> &str {
            match self {
                TestState::Initial => "initial",
                TestState::Processing => "processing",
                TestState::Final => "final",
            }
        }
        
        fn state_type(&self) -> &rustate::StateType {
            static NORMAL: rustate::StateType = rustate::StateType::Normal;
            &NORMAL
        }
        
        fn parent(&self) -> Option<&str> {
            None
        }
        
        fn children(&self) -> &[String] {
            &[]
        }
        
        fn initial(&self) -> Option<&str> {
            None
        }
        
        fn data(&self) -> Option<&serde_json::Value> {
            None
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestEvent {
        Start,
        Process,
        Finish,
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "start",
                TestEvent::Process => "process",
                TestEvent::Finish => "finish",
            }
        }
        
        fn payload(&self) -> Option<&serde_json::Value> {
            None
        }
    }

    #[test]
    fn test_observation_creation() {
        let obs = Observation::new(
            TestState::Initial,
            TestEvent::Start,
            TestState::Processing,
        );

        assert_eq!(obs.previous_state, TestState::Initial);
        assert_eq!(obs.event, TestEvent::Start);
        assert_eq!(obs.next_state, TestState::Processing);
        assert!(obs.metadata.is_empty());
    }

    #[test]
    fn test_observation_with_metadata() {
        let obs = Observation::new(
            TestState::Processing,
            TestEvent::Finish,
            TestState::Final,
        )
        .with_metadata("user", "test_user")
        .with_metadata("source", "test_case");

        assert_eq!(obs.previous_state, TestState::Processing);
        assert_eq!(obs.event, TestEvent::Finish);
        assert_eq!(obs.next_state, TestState::Final);
        assert_eq!(obs.metadata.len(), 2);
        assert_eq!(obs.metadata.get("user"), Some(&"test_user".to_string()));
        assert_eq!(obs.metadata.get("source"), Some(&"test_case".to_string()));
    }
} 