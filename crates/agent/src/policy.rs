use crate::{
    decision::{Decision, DecisionContext},
    error::{AgentError, Result},
    feedback::Feedback,
    insight::Insight,
    observation::Observation,
};
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
    async fn decide(&self, context: DecisionContext<S, E>) -> Result<Decision<E>>;

    /// フィードバックに応じてポリシーを更新します
    fn update(&self, _feedback: Feedback<E>) {
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

    async fn decide(&self, context: DecisionContext<S, E>) -> Result<Decision<E>> {
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
            Some(context.current_state.clone()),
            Some(context.goal_state.clone()),
        ))
    }
}

/// ヒューリスティックルールによって決定を行うポリシー
pub struct HeuristicPolicy<S, E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static + Clone,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    rules: Vec<Box<dyn HeuristicRule<S, E> + Send + Sync>>,
    fallback_policy: Box<dyn Policy<S, E> + Send + Sync>,
    name: String,
    description: String,
}

impl<S, E> HeuristicPolicy<S, E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static + Clone,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    pub fn new(fallback_policy: impl Policy<S, E> + Send + Sync + 'static) -> Self {
        Self {
            rules: Vec::new(),
            fallback_policy: Box::new(fallback_policy),
            name: "ヒューリスティックポリシー".to_string(),
            description: "ルールベースのヒューリスティックを使用する決定ポリシー".to_string(),
        }
    }

    pub fn add_rule(mut self, rule: impl HeuristicRule<S, E> + Send + Sync + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
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

/// ヒューリスティックルールのトレイト
pub trait HeuristicRule<S, E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static + Clone,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    /// ルールの名前
    fn name(&self) -> &str;

    /// ルールの優先度（高いほど優先）
    fn priority(&self) -> i32;

    /// 状態に対してこのルールが適用可能かどうかを判断
    fn matches(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> bool;

    /// ルールが生成する決定のイベント
    fn get_event(&self, current_state: &S, goal_state: Option<&S>) -> E;

    /// ルールの信頼度
    fn confidence(&self) -> f64;
}

#[async_trait]
impl<S, E> Policy<S, E> for HeuristicPolicy<S, E>
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

    async fn decide(&self, context: DecisionContext<S, E>) -> Result<Decision<E>> {
        // Apply each rule in priority order
        let mut applicable_rules: Vec<&Box<dyn HeuristicRule<S, E> + Send + Sync>> = self
            .rules
            .iter()
            .filter(|rule| {
                rule.matches(
                    &context.current_state,
                    Some(&context.goal_state),
                    &context.observations,
                    &context.insights,
                )
            })
            .collect();

        // Sort by priority (highest first)
        applicable_rules.sort_by(|a, b| b.priority().cmp(&a.priority()));

        // Apply the highest priority rule
        if let Some(rule) = applicable_rules.first() {
            let event = rule.get_event(&context.current_state, Some(&context.goal_state));
            let confidence = rule.confidence();

            return Ok(Decision::new(
                uuid::Uuid::new_v4().to_string(),
                event,
                confidence,
                Some(context.current_state.clone()),
                Some(context.goal_state.clone()),
            ));
        }

        // If no rule applies, use the fallback policy
        self.fallback_policy.decide(context).await
    }
}

/// Arcの中にPolicyトレイトを実装した型を格納するための型エイリアス
pub type PolicyBox<S, E> = Arc<dyn Policy<S, E>>;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::decision_context::DecisionContext;
    use crate::error::AgentError;
    use crate::feedback::Feedback;
    use crate::types::{Insight, Observation};
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
                TestState::Initial => "initial",
                TestState::Processing => "processing",
                TestState::Final => "final",
            }
        }

        fn state_type(&self) -> &StateType {
            static STATE_TYPE: StateType = StateType::Normal;
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
        Mock,
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "start",
                TestEvent::Process => "process",
                TestEvent::Finish => "finish",
                TestEvent::Mock => "mock",
            }
        }

        fn payload(&self) -> Option<&Value> {
            None
        }
    }

    #[tokio::test]
    async fn test_random_policy() {
        let events = vec![TestEvent::Start, TestEvent::Process, TestEvent::Finish];

        let policy = RandomPolicy::new(events).with_name("テストランダムポリシー");

        // 明示的に型パラメータを指定
        assert_eq!(
            Policy::<TestState, TestEvent>::name(&policy),
            "テストランダムポリシー"
        );

        let current_state = TestState::Initial;
        // Create empty vectors for observations, feedbacks, insights
        let observations: Vec<Observation<TestState, TestEvent>> = Vec::new();
        let feedbacks: Vec<Feedback<TestEvent>> = Vec::new();
        let insights: Vec<Insight> = Vec::new();

        let decision = policy
            .decide(DecisionContext::new(
                current_state,
                None,         // goal_state is Option<S>
                observations, // Pass Vec directly
                feedbacks,    // Pass Vec directly
                insights,     // Pass Vec directly
            ))
            .await
            .unwrap();

        assert!(matches!(
            decision.event,
            TestEvent::Start | TestEvent::Process | TestEvent::Finish
        ));
        assert!(decision.confidence >= 0.5 && decision.confidence <= 1.0);
    }

    struct MockPolicy;

    #[async_trait::async_trait]
    impl Policy<TestState, TestEvent> for MockPolicy {
        async fn decide(
            &self,
            context: DecisionContext<TestState, TestEvent>,
        ) -> Result<Decision<TestEvent>, AgentError> {
            // Simple mock: always decide to send a "MOCK_EVENT"
            Ok(Decision::new(
                uuid::Uuid::new_v4().to_string(),
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
        let current_state = TestState::Initial;
        let goal_state = TestState::Final;
        let observations = vec![];
        let feedbacks = vec![];
        let insights = vec![];

        // Provide all 5 arguments to DecisionContext::new
        let context = DecisionContext::new(
            current_state,
            Some(goal_state), // Pass Option<S>
            observations,     // Pass Vec directly
            feedbacks,        // Pass Vec directly
            insights,         // Pass Vec directly
        );

        let decision_result = policy.decide(context).await;

        assert!(decision_result.is_ok());
        let decision = decision_result.unwrap();
        assert_eq!(decision.event, TestEvent::Mock);
    }

    // ... other tests ...
}
