use crate::agent::AgentId;
use crate::decision::{Decision, DecisionContext};
use crate::error::PolicyError;
use async_trait::async_trait;
use rustate::{EventTrait, StateTrait};
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug, Display, Formatter};
use std::sync::Arc;
use rand::seq::SliceRandom;
use uuid::Uuid;

/// ポリシートレイト - エージェントの決定プロセスを定義します
#[async_trait]
pub trait Policy<S, E>: Send + Sync
where
    S: StateTrait + Clone + Debug + Send + Sync + 'static + DeserializeOwned,
    E: EventTrait + Clone + Debug + Send + Sync + 'static + DeserializeOwned,
{
    /// ポリシーの名前を返します
    fn name(&self) -> &str;

    /// ポリシーの説明を返します
    fn description(&self) -> &str;

    /// Provide a decision based on the given context
    async fn decide(
        &self,
        context: &DecisionContext<S, E>,
    ) -> Result<Decision<E>, PolicyError>;

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

    async fn decide(
        &self,
        context: &DecisionContext<S, E>,
    ) -> Result<Decision<E>, PolicyError> {
        let mut rng = rand::thread_rng();

        if self.available_events.is_empty() {
            return Err(PolicyError::NoPossibleEvents);
        }

        let event =
            self.available_events
                .choose(&mut rng)
                .cloned()
                .ok_or(PolicyError::DecisionFailed(
                    "Failed to choose event".to_string(),
                ))?;

        Ok(Decision::new(
            Uuid::new_v4().to_string(),
            event,
            0.5,
            Some(context.current_state.clone()),
            Some(context.goal_state.clone()),
        ))
    }
}

/// Arcの中にPolicyトレイトを実装した型を格納するための型エイリアス
pub type PolicyBox<S, E> = Arc<dyn Policy<S, E>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::DecisionContext;
    use rustate::{StateTrait, EventTrait};
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Display, Formatter};
    use uuid::Uuid;
    use serde_json::Value;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl Display for TestState {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &Self {
            self
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
            match self {
                TestEvent::Start => "START",
                TestEvent::Process => "PROCESS",
                TestEvent::Finish => "FINISH",
                TestEvent::Mock => "MOCK",
            }
        }
    }

    #[tokio::test]
    async fn test_random_policy() {
        let events = vec![TestEvent::Start, TestEvent::Process, TestEvent::Finish];
        let policy = RandomPolicy::new(events.clone());

        let context = DecisionContext {
            current_state: TestState::Initial,
            goal_state: TestState::Final,
            observations: vec![],
            feedbacks: vec![],
            insights: vec![],
        };

        let decision = policy.decide(&context).await.unwrap();

        assert!(events.contains(&decision.event));
        assert_eq!(decision.confidence, 0.5);
        assert!(decision.id.len() > 0);
        assert_eq!(decision.state_context, Some("Initial".to_string()));
        assert_eq!(decision.goal_context, Some("Final".to_string()));
    }

    struct MockPolicy;

    #[async_trait]
    impl Policy<TestState, TestEvent> for MockPolicy {
        fn name(&self) -> &str {
            "Mock Policy"
        }

        fn description(&self) -> &str {
            "A simple mock policy for testing"
        }

        async fn decide(
            &self,
            context: &DecisionContext<TestState, TestEvent>,
        ) -> Result<Decision<TestEvent>, PolicyError> {
            Ok(Decision::new(
                Uuid::new_v4().to_string(),
                TestEvent::Mock,
                1.0,
                Some(context.current_state.clone()),
                Some(context.goal_state.clone()),
            ))
        }
    }

    #[tokio::test]
    async fn test_simple_policy_decide() {
        let policy = MockPolicy;
        let context = DecisionContext {
            current_state: TestState::Initial,
            goal_state: TestState::Final,
            observations: vec![],
            feedbacks: vec![],
            insights: vec![],
        };
        let decision = policy.decide(&context).await.unwrap();

        assert_eq!(decision.event, TestEvent::Mock);
        assert_eq!(decision.confidence, 1.0);
    }
}
