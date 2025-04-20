use crate::{feedback::Feedback, observation::Observation, AgentError};
use crate::prelude::Result;
use async_trait::async_trait;
use rustate::{Event, State, StateTrait, EventTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::{self, Debug};

/// エージェントの決定を表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision<E: Event> {
    /// 決定の一意な識別子
    pub id: String,

    /// 選択されたイベント
    pub event: E,

    /// 決定が行われた理由の説明
    pub reasoning: String,

    /// この決定に関連する追加のメタデータ
    pub metadata: HashMap<String, String>,

    /// この決定が作成された時間（UNIXタイムスタンプ）
    pub timestamp: u64,

    /// この決定に関連するフィードバック
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<Feedback>,
}

impl<E: Event> Decision<E> {
    /// 新しい決定を作成します
    pub fn new(event: E, reasoning: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            event,
            reasoning: reasoning.into(),
            metadata: HashMap::new(),
            timestamp: current_timestamp(),
            feedback: None,
        }
    }

    /// 決定にメタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 決定に複数のメタデータを一度に追加します
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    /// 決定にフィードバックを追加します
    pub fn with_feedback(mut self, feedback: Feedback) -> Self {
        self.feedback = Some(feedback);
        self
    }
}

/// 決定を行うコンポーネントのトレイト
#[async_trait]
pub trait DecisionMaker<S, E>
where
    S: State,
    E: Event,
{
    /// 現在の状態、目標状態、過去の観測データに基づいて決定を行います
    async fn make_decision(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
    ) -> Result<Decision<E>>;
}

/// 現在のUNIXタイムスタンプを返します
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// 決定用の一意な識別子を生成します
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp();
    format!("dec-{}-{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustate::{EventTrait, StateTrait};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestEvent {
        Start,
        Process,
        Finish,
    }

    impl EventTrait for TestEvent {}

    #[test]
    fn test_decision_creation() {
        let decision = Decision::new(TestEvent::Start, "ユーザーがリクエストしたため");

        assert_eq!(decision.event, TestEvent::Start);
        assert_eq!(decision.reasoning, "ユーザーがリクエストしたため");
        assert!(decision.metadata.is_empty());
        assert!(decision.feedback.is_none());
    }

    #[test]
    fn test_decision_with_metadata() {
        let decision = Decision::new(TestEvent::Process, "処理を開始する必要があるため")
            .with_metadata("priority", "high")
            .with_metadata("source", "user input");

        assert_eq!(decision.metadata.get("priority"), Some(&"high".to_string()));
        assert_eq!(decision.metadata.get("source"), Some(&"user input".to_string()));
    }
} 