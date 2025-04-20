use rustate::{Event, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// 観測データは、状態遷移に関する情報を記録します。
/// 前の状態、イベント、結果の状態、メタデータなどを含みます。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation<S, E>
where
    S: State,
    E: Event,
{
    /// 観測の一意な識別子
    pub id: String,

    /// 前の状態
    pub previous_state: S,

    /// 適用されたイベント
    pub event: E,

    /// 結果の状態
    pub resulting_state: S,

    /// この観測が記録された時間（UNIXタイムスタンプ）
    pub timestamp: u64,

    /// この観測に関連する追加のメタデータ
    pub metadata: HashMap<String, String>,
}

impl<S, E> Observation<S, E>
where
    S: State,
    E: Event,
{
    /// 新しい観測を作成します
    pub fn new(previous_state: S, event: E, resulting_state: S) -> Self {
        Self {
            id: uuid(),
            previous_state,
            event,
            resulting_state,
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }

    /// 観測にメタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 観測に複数のメタデータを一度に追加します
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

/// シンプルなUUID v4互換の識別子を生成します
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
    use rustate::{StateTrait, EventTrait};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl StateTrait for TestState {}

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestEvent {
        Start,
        Process,
        Finish,
    }

    impl EventTrait for TestEvent {}

    #[test]
    fn test_observation_creation() {
        let obs = Observation::new(
            TestState::Initial,
            TestEvent::Start,
            TestState::Processing,
        );

        assert_eq!(obs.previous_state, TestState::Initial);
        assert_eq!(obs.event, TestEvent::Start);
        assert_eq!(obs.resulting_state, TestState::Processing);
        assert!(obs.metadata.is_empty());
    }

    #[test]
    fn test_observation_with_metadata() {
        let obs = Observation::new(
            TestState::Initial,
            TestEvent::Start,
            TestState::Processing,
        )
        .with_metadata("reason", "user requested")
        .with_metadata("confidence", "high");

        assert_eq!(obs.metadata.get("reason"), Some(&"user requested".to_string()));
        assert_eq!(obs.metadata.get("confidence"), Some(&"high".to_string()));
    }
} 