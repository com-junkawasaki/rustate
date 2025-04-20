use crate::{feedback::Feedback, observation::Observation, AgentError};
use crate::prelude::Result;
use async_trait::async_trait;
use rustate::{Event, State, StateTrait, EventTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};

/// エージェントの決定を表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision<E: EventTrait + Clone> {
    /// 一意の決定ID
    pub id: String,
    /// 決定のタイムスタンプ
    pub timestamp: u64,
    /// 決定されたイベント
    pub event: E,
    /// 決定の信頼度 (0.0-1.0)
    pub confidence: f64,
    /// 追加のメタデータ
    pub metadata: HashMap<String, String>,
}

/// 決定の新規作成と管理のメソッド
impl<E: EventTrait + Clone> Decision<E> {
    /// 新しい決定を作成します
    pub fn new(event: E, confidence: f64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("時間が取得できませんでした")
            .as_secs();
        
        Self {
            id: format!("decision-{}", uuid::Uuid::new_v4()),
            timestamp,
            event,
            confidence,
            metadata: HashMap::new(),
        }
    }
    
    /// メタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// 決定の信頼度を設定します
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.max(0.0).min(1.0);
        self
    }
}

/// 決定を作成するためのトレイト
#[async_trait]
pub trait DecisionMaker<S, E>
where
    S: StateTrait + Clone,
    E: EventTrait + Clone,
{
    /// 現在の状態と目標状態から次の決定を行います
    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[crate::insight::Insight],
    ) -> Result<Decision<E>>;
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
        let decision = Decision::new(TestEvent::Start, 0.8);

        assert_eq!(decision.event, TestEvent::Start);
        assert_eq!(decision.confidence, 0.8);
        assert!(decision.metadata.is_empty());
    }

    #[test]
    fn test_decision_with_metadata() {
        let decision = Decision::new(TestEvent::Process, 0.8)
            .with_metadata("priority", "high")
            .with_metadata("source", "user input");

        assert_eq!(decision.metadata.get("priority"), Some(&"high".to_string()));
        assert_eq!(decision.metadata.get("source"), Some(&"user input".to_string()));
    }
} 