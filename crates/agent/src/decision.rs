use crate::{feedback::Feedback, observation::Observation, insight::Insight, AgentError};
use crate::prelude::Result;
use async_trait::async_trait;
use rustate::{EventTrait, StateTrait};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::Value;

/// エージェントの決定の文脈を表す構造体
#[derive(Clone, Debug)]
pub struct DecisionContext<'a, S, E>
where
    S: StateTrait + Debug + Send + Sync + DeserializeOwned + 'static,
    E: EventTrait + Debug + Send + Sync + DeserializeOwned + 'static,
{
    /// 現在の状態
    pub current_state: S,
    /// 目標状態（オプション）
    pub goal_state: Option<S>,
    /// 観測データの参照
    pub observations: &'a [Observation<S, E>],
    /// インサイト（洞察）の参照
    pub insights: &'a [Insight],
}

impl<'a, S, E> DecisionContext<'a, S, E>
where
    S: StateTrait + Debug + Send + Sync + DeserializeOwned + 'static,
    E: EventTrait + Debug + Send + Sync + DeserializeOwned + 'static,
{
    /// 新しい決定文脈を作成します
    pub fn new(
        current_state: S,
        goal_state: Option<S>,
        observations: &'a [Observation<S, E>],
        insights: &'a [Insight],
    ) -> Self {
        Self {
            current_state,
            goal_state,
            observations,
            insights,
        }
    }
}

/// エージェントの決定を表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "E: Serialize + for<'deserialize> Deserialize<'deserialize>")]
pub struct Decision<E>
where
    E: EventTrait + Debug + Send + Sync + 'static,
{
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
impl<E> Decision<E>
where
    E: EventTrait + Debug + Send + Sync + 'static,
{
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
    S: StateTrait + Debug + Send + Sync + DeserializeOwned + 'static,
    E: EventTrait + Debug + Send + Sync + DeserializeOwned + 'static,
{
    /// 現在の状態と目標状態から次の決定を行います
    async fn decide(
        &self,
        context: DecisionContext<'_, S, E>,
    ) -> Result<Decision<E>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustate::{EventTrait, StateTrait};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
            static STATE_TYPE: rustate::StateType = rustate::StateType::Normal;
            &STATE_TYPE
        }
        
        fn parent(&self) -> Option<&str> {
            None
        }
        
        fn children(&self) -> &[String] {
            static EMPTY: [String; 0] = [];
            &EMPTY
        }
        
        fn initial(&self) -> Option<&str> {
            None
        }
        
        fn data(&self) -> Option<&Value> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        
        fn payload(&self) -> Option<&Value> {
            None
        }
    }

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