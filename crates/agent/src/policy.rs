use crate::error::AgentError;
use crate::{decision::Decision, error::Result};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rustate::{EventTrait, StateTrait};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::marker::Send;
use std::marker::Sync;
use std::sync::Arc;
use uuid;

/// ポリシートレイト - エージェントの決定プロセスを定義します
#[async_trait]
pub trait Policy<S, E>: Send + Sync
where
    S: StateTrait + Clone + Debug + Send + Sync + 'static,
    E: EventTrait + Clone + Debug + Send + Sync + 'static,
{
    /// ポリシーの名前を返します
    fn name(&self) -> &str {
        "基本ポリシー"
    }

    /// ポリシーの説明を返します
    fn description(&self) -> &str {
        "基本的な決定ポリシー"
    }

    /// 現在の状態、目標、過去の観測などに基づいて次のアクションを決定します
    /// 現在の状態と文脈に基づいて決定を行います
    async fn decide(&self, current_state: S, goal_state: S) -> Result<Decision<E>>;

    /// フィードバックに応じてポリシーを更新します
    fn update(&self, _event: E) {
        // デフォルトでは何もしません
    }
}

/// 利用可能なイベントからランダムに選択するシンプルなポリシー
pub struct RandomPolicy<E>
where
    E: EventTrait + Clone + Debug + Send + Sync + DeserializeOwned + 'static,
{
    available_events: Vec<E>,
    name: String,
    description: String,
}

impl<E> RandomPolicy<E>
where
    E: EventTrait + Clone + Debug + Send + Sync + DeserializeOwned + 'static,
{
    pub fn new(available_events: Vec<E>) -> Self {
        Self {
            available_events,
            name: "ランダムポリシー".to_string(),
            description: "利用可能なイベントからランダムに選択する決定ポリシー".to_string(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

#[async_trait]
impl<S, E> Policy<S, E> for RandomPolicy<E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static + Clone,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn decide(&self, current_state: S, goal_state: S) -> Result<Decision<E>> {
        let mut rng = rand::thread_rng();

        if self.available_events.is_empty() {
            return Err(AgentError::PolicyError(
                "利用可能なイベントがありません".to_string(),
            ));
        }

        let event = self
            .available_events
            .choose(&mut rng)
            .cloned()
            .ok_or_else(|| AgentError::PolicyError("イベント選択エラー".to_string()))?;

        Ok(Decision::new(
            uuid::Uuid::new_v4().to_string(),
            event,
            0.5, // ランダム選択なので信頼度は中程度
            Some(current_state.clone()),
            Some(goal_state.clone()),
        ))
    }
}

/// Arcの中にPolicyトレイトを実装した型を格納するための型エイリアス
pub type PolicyBox<S, E> = Arc<dyn Policy<S, E>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AgentError;
    use rustate::{EventTrait, StateTrait, StateType};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl StateTrait for TestState {
        fn id(&self) -> &str {
            match self {
                TestState::Initial => "Initial",
                TestState::Processing => "Processing",
                TestState::Final => "Final",
            }
        }

        fn state_type(&self) -> &StateType {
            match self {
                TestState::Final => &StateType::Final,
                _ => &StateType::Normal,
            }
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

        fn data(&self) -> Option<&Value> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestEvent {
        Start,
        Process,
        Finish,
        Mock,
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "Start",
                TestEvent::Process => "Process",
                TestEvent::Finish => "Finish",
                TestEvent::Mock => "Mock",
            }
        }

        fn payload(&self) -> Option<&Value> {
            None
        }

        fn name(&self) -> &str {
            self.event_type()
        }
    }

    #[tokio::test]
    async fn test_random_policy() {
        let events = vec![TestEvent::Start, TestEvent::Process, TestEvent::Finish];
        let policy = RandomPolicy::new(events.clone());

        let decision = policy
            .decide(TestState::Initial, TestState::Final)
            .await
            .unwrap();

        assert!(events.contains(&decision.event));
        assert_eq!(decision.confidence, 0.5);
        assert!(decision.id.len() > 0);
        assert_eq!(decision.state_context, Some("Initial".to_string()));
        assert_eq!(decision.goal_context, Some("Final".to_string()));
    }

    struct MockPolicy;

    #[async_trait]
    impl Policy<TestState, TestEvent> for MockPolicy {
        async fn decide(
            &self,
            _current_state: TestState,
            _goal_state: TestState,
        ) -> Result<Decision<TestEvent>> {
            Ok(Decision::new(
                uuid::Uuid::new_v4().to_string(),
                TestEvent::Mock,
                1.0,
                Some(TestState::Initial),
                Some(TestState::Final),
            ))
        }
    }

    #[tokio::test]
    async fn test_simple_policy_decide() {
        let policy = MockPolicy;
        let decision = policy
            .decide(TestState::Initial, TestState::Final)
            .await
            .unwrap();

        assert_eq!(decision.event, TestEvent::Mock);
        assert_eq!(decision.confidence, 1.0);
    }
}
