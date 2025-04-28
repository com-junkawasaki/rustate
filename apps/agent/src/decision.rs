use crate::feedback::Feedback;
use crate::insight::Insight;
use crate::observation::Observation;
use rustate::{Event, State, StateType};
use rustate::{EventTrait, StateTrait};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display, Formatter};
use std::time::SystemTime;

/// エージェントが行う決定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "E: EventTrait + DeserializeOwned"))]
pub struct Decision<E>
where
    E: EventTrait + Clone + Debug + DeserializeOwned,
{
    /// 決定の一意ID
    pub id: String,
    /// 決定されたイベント
    pub event: E,
    /// 決定の信頼度（0.0 - 1.0）
    pub confidence: f64,
    /// 決定時の状態（オプション）
    pub state_context: Option<String>,
    /// 決定時のゴール状態（オプション）
    pub goal_context: Option<String>,
    /// 決定の説明理由（オプション）
    pub explanation: Option<String>,
    /// 決定が行われたタイムスタンプ
    pub timestamp: SystemTime,
    /// メタデータ
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl<E> Decision<E>
where
    E: EventTrait + Clone + Debug + DeserializeOwned,
{
    /// 新しい決定を作成します
    pub fn new(
        id: impl Into<String>,
        event: E,
        confidence: f64,
        state_context: Option<impl StateTrait>,
        goal_context: Option<impl StateTrait>,
    ) -> Self {
        Self {
            id: id.into(),
            event,
            confidence,
            state_context: state_context.map(|s| s.id().to_string()),
            goal_context: goal_context.map(|s| s.id().to_string()),
            explanation: None,
            timestamp: SystemTime::now(),
            metadata: serde_json::Map::new(),
        }
    }

    /// 説明を追加します
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    /// メタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let key = key.into();
        if let Ok(value) = serde_json::to_value(value) {
            self.metadata.insert(key, value);
        }
        self
    }

    /// 決定の信頼度を取得します
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// 決定されたイベントを参照で取得します
    pub fn event(&self) -> &E {
        &self.event
    }

    /// 決定のIDを取得します
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 決定のタイムスタンプを取得します
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }

    // For tests, a simpler constructor that just takes a UUID string and event
    pub fn simple(event: E, confidence: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event,
            confidence,
            state_context: None,
            goal_context: None,
            explanation: None,
            timestamp: SystemTime::now(),
            metadata: serde_json::Map::new(),
        }
    }
}

/// 決定を行うためのコンテキスト情報
#[derive(Debug, Clone)]
pub struct DecisionContext<S, E>
where
    S: StateTrait + Clone + Debug,
    E: EventTrait + Clone + Debug + DeserializeOwned,
{
    /// 現在の状態
    pub current_state: S,
    /// 目標状態
    pub goal_state: S,
    /// 過去の観測
    pub observations: Vec<Observation<S, E>>,
    /// 過去のフィードバック
    pub feedbacks: Vec<Feedback<E>>,
    /// 洞察
    pub insights: Vec<Insight>,
}

impl<S, E> DecisionContext<S, E>
where
    S: StateTrait + Clone + Debug,
    E: EventTrait + Clone + Debug + DeserializeOwned,
{
    /// 新しい決定コンテキストを作成します
    pub fn new(
        current_state: S,
        goal_state: S,
        observations: Vec<Observation<S, E>>,
        feedbacks: Vec<Feedback<E>>,
        insights: Vec<Insight>,
    ) -> Self {
        Self {
            current_state,
            goal_state,
            observations,
            feedbacks,
            insights,
        }
    }

    /// 過去の観測から状態遷移の履歴を取得します
    pub fn state_history(&self) -> Vec<&S> {
        self.observations
            .iter()
            .map(|o| &o.previous_state)
            .collect()
    }

    /// 目標状態までの最短パスを推定します（実装例）
    pub fn estimate_path_to_goal(&self) -> Vec<String> {
        // 実際の実装では、過去の観測や状態遷移グラフを使用して
        // 現在の状態から目標状態までの最短パスを推定します
        // ここでは簡単な例として現在と目標の状態IDを返します
        vec![
            self.current_state.id().to_string(),
            self.goal_state.id().to_string(),
        ]
    }

    /// 観測から成功する可能性が高いアクションを推定します
    pub fn suggest_actions_from_observations(&self) -> Vec<&E> {
        // 過去の観測から、目標状態に近づいた成功したアクションを抽出
        self.observations.iter().map(|o| &o.event).collect()
    }
}

/// 決定を作成するトレイト
pub trait DecisionMaker<S, E>
where
    S: StateTrait + Clone + Debug,
    E: EventTrait + Clone + Debug + DeserializeOwned,
{
    /// コンテキストに基づいて決定を行います
    fn make_decision(&self, context: DecisionContext<S, E>) -> Decision<E>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::fmt::{self, Display, Formatter};
    use std::hash::Hash;

    // テスト用の状態
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct TestState {
        id: String,
    }

    // Add Display impl for TestState
    impl Display for TestState {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.id)
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &Self {
            self
        }
    }

    // テスト用のイベント
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct TestEvent {
        event_type: String,
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            &self.event_type
        }

        fn payload(&self) -> Option<&Value> {
            None
        }

        // Implement the missing name method
        fn name(&self) -> &str {
            self.event_type()
        }
    }

    impl rustate::IntoEvent for TestEvent {
        fn into_event(self) -> Event {
            Event::new(self.event_type())
        }
    }

    #[test]
    fn test_decision_creation() {
        let event = TestEvent {
            event_type: "TEST_EVENT".to_string(),
        };

        let current_state = TestState {
            id: "current".to_string(),
        };

        let goal_state = TestState {
            id: "goal".to_string(),
        };

        let decision = Decision::new(
            "test-decision-1",
            event.clone(),
            0.9,
            Some(current_state.clone()),
            Some(goal_state.clone()),
        );

        assert_eq!(decision.id(), "test-decision-1");
        assert_eq!(decision.event().event_type, "TEST_EVENT");
        assert_eq!(decision.confidence(), 0.9);
        assert_eq!(decision.state_context, Some("current".to_string()));
        assert_eq!(decision.goal_context, Some("goal".to_string()));
    }

    #[test]
    fn test_decision_with_metadata() {
        let event = TestEvent {
            event_type: "TEST_EVENT".to_string(),
        };

        let decision = Decision::new(
            "test-decision-2",
            event,
            0.8,
            None as Option<TestState>,
            None as Option<TestState>,
        )
        .with_metadata("key1", "value1")
        .with_metadata("key2", 42);

        assert!(decision.metadata.contains_key("key1"));
        assert!(decision.metadata.contains_key("key2"));

        if let Some(serde_json::Value::String(v)) = decision.metadata.get("key1") {
            assert_eq!(v, "value1");
        } else {
            panic!("Expected String value for key1");
        }

        if let Some(serde_json::Value::Number(v)) = decision.metadata.get("key2") {
            assert_eq!(v.as_i64().unwrap(), 42);
        } else {
            panic!("Expected Number value for key2");
        }
    }

    #[test]
    fn test_decision_context() {
        let current_state = TestState {
            id: "current".to_string(),
        };
        let goal_state = TestState {
            id: "goal".to_string(),
        };
        let event = TestEvent {
            event_type: "START".to_string(),
        };

        let observation = Observation::new(
            TestState {
                id: "prev".to_string(),
            },
            event.clone(),
            current_state.clone(),
        );
        let feedback = Feedback::new("Good job", crate::feedback::FeedbackType::Positive, "user");

        // Add type annotation for E
        let context = DecisionContext::<TestState, TestEvent>::new(
            current_state.clone(), // Clone if needed, as current_state is used later
            goal_state.clone(),    // Clone if needed
            vec![observation],
            vec![feedback],
            vec![],
        );

        assert_eq!(
            context.current_state,
            TestState {
                id: "current".to_string()
            }
        );
        assert_eq!(context.observations.len(), 1);
        assert_eq!(context.feedbacks.len(), 1);
        assert_eq!(context.insights.len(), 0);
    }
}
