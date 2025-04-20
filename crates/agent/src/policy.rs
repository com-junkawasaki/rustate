use crate::{
    decision::{Decision, DecisionMaker},
    error::Result,
    insight::Insight,
    observation::Observation,
};
use async_trait::async_trait;
use rustate::{Event, State};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

/// エージェントの決定ポリシーのトレイト
/// ポリシーは、エージェントが決定を行う際の戦略を定義します。
#[async_trait]
pub trait Policy<S, E>: Send + Sync
where
    S: State + DeserializeOwned + 'static,
    E: Event + DeserializeOwned + 'static,
{
    /// 名前を返します
    fn name(&self) -> &str;

    /// 説明を返します
    fn description(&self) -> &str;

    /// 現在の状態と目標状態に基づいて決定を行います
    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> Result<Decision<E>>;
}

/// ランダムポリシー - 利用可能なイベントからランダムに選択します。
/// テストと基本的なエージェント動作のデモに適しています。
pub struct RandomPolicy<E: Event> {
    name: String,
    description: String,
    available_events: Vec<E>,
}

impl<E: Event> RandomPolicy<E> {
    pub fn new(available_events: Vec<E>) -> Self {
        Self {
            name: "ランダムポリシー".to_string(),
            description: "利用可能なイベントからランダムに選択するシンプルなポリシー".to_string(),
            available_events,
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
    S: State + DeserializeOwned + Debug + 'static,
    E: Event + DeserializeOwned + Clone + Debug + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        _observations: &[Observation<S, E>],
        _insights: &[Insight],
    ) -> Result<Decision<E>> {
        use crate::error::AgentError;
        use rand::seq::SliceRandom;

        if self.available_events.is_empty() {
            return Err(AgentError::DecisionError(
                "利用可能なイベントがありません".to_string(),
            ));
        }

        let rng = &mut rand::thread_rng();
        let selected_event = self
            .available_events
            .choose(rng)
            .ok_or_else(|| AgentError::DecisionError("イベントの選択に失敗しました".to_string()))?
            .clone();

        let reasoning = format!(
            "現在の状態: {:?}、目標状態: {:?}から、ランダムに選択しました",
            current_state, goal_state
        );

        Ok(Decision::new(selected_event, reasoning))
    }
}

/// ヒューリスティックポリシー - 事前定義されたルールに基づいて決定を行います。
pub struct HeuristicPolicy<S, E> {
    name: String,
    description: String,
    rules: Vec<Box<dyn HeuristicRule<S, E> + Send + Sync>>,
}

impl<S, E> HeuristicPolicy<S, E>
where
    S: State,
    E: Event,
{
    pub fn new() -> Self {
        Self {
            name: "ヒューリスティックポリシー".to_string(),
            description: "事前定義されたルールに基づいて決定を行うポリシー".to_string(),
            rules: Vec::new(),
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

    pub fn add_rule(mut self, rule: impl HeuristicRule<S, E> + Send + Sync + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }
}

#[async_trait]
impl<S, E> Policy<S, E> for HeuristicPolicy<S, E>
where
    S: State + DeserializeOwned + Debug + 'static,
    E: Event + DeserializeOwned + Clone + Debug + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> Result<Decision<E>> {
        use crate::error::AgentError;

        if self.rules.is_empty() {
            return Err(AgentError::DecisionError(
                "ルールが定義されていません".to_string(),
            ));
        }

        // 最も高いスコアを持つルールを見つける
        let mut best_rule: Option<(usize, f64)> = None;
        let mut scores = Vec::new();

        for (i, rule) in self.rules.iter().enumerate() {
            let score = rule.evaluate(current_state, goal_state, observations, insights);
            scores.push((i, score));

            match best_rule {
                None => best_rule = Some((i, score)),
                Some((_, best_score)) if score > best_score => best_rule = Some((i, score)),
                _ => {}
            }
        }

        // 適用するルールを決定
        let (rule_index, score) = best_rule.ok_or_else(|| {
            AgentError::DecisionError("ルールの評価に失敗しました".to_string())
        })?;

        let rule = &self.rules[rule_index];
        let event = rule.get_event(current_state, goal_state);
        let reasoning = format!(
            "ルール「{}」が最高スコア({:.2})で選択されました。{:?} から {:?} への最適なイベントです。",
            rule.name(),
            score,
            current_state,
            goal_state
        );

        Ok(Decision::new(event, reasoning).with_metadata("rule_score", score.to_string()))
    }
}

/// ヒューリスティックルールのトレイト
#[async_trait]
pub trait HeuristicRule<S, E>
where
    S: State,
    E: Event,
{
    /// ルールの名前を返します
    fn name(&self) -> &str;

    /// 現在の状態、目標状態、観測データに基づいてルールを評価し、スコアを返します
    fn evaluate(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> f64;

    /// 適用する実際のイベントを返します
    fn get_event(&self, current_state: &S, goal_state: Option<&S>) -> E;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AgentError;
    use rustate::{EventTrait, StateTrait};
    use serde::{Deserialize, Serialize};

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

    struct TestRule {
        name: String,
        target_state: TestState,
        output_event: TestEvent,
    }

    #[async_trait]
    impl HeuristicRule<TestState, TestEvent> for TestRule {
        fn name(&self) -> &str {
            &self.name
        }

        fn evaluate(
            &self,
            current_state: &TestState,
            goal_state: Option<&TestState>,
            _observations: &[Observation<TestState, TestEvent>],
            _insights: &[Insight],
        ) -> f64 {
            match goal_state {
                Some(goal) if *goal == self.target_state => 1.0,
                _ if *current_state != self.target_state => 0.5,
                _ => 0.0,
            }
        }

        fn get_event(&self, _current_state: &TestState, _goal_state: Option<&TestState>) -> TestEvent {
            self.output_event.clone()
        }
    }

    #[tokio::test]
    async fn test_random_policy() {
        let events = vec![TestEvent::Start, TestEvent::Process, TestEvent::Finish];
        let policy = RandomPolicy::new(events)
            .with_name("テストランダムポリシー")
            .with_description("テスト用");

        assert_eq!(policy.name(), "テストランダムポリシー");
        
        let current_state = TestState::Initial;
        let goal_state = Some(TestState::Final);
        let observations: Vec<Observation<TestState, TestEvent>> = Vec::new();
        let insights: Vec<Insight> = Vec::new();

        let decision = policy
            .decide(&current_state, goal_state.as_ref(), &observations, &insights)
            .await
            .unwrap();

        assert!(matches!(
            decision.event,
            TestEvent::Start | TestEvent::Process | TestEvent::Finish
        ));
    }

    #[tokio::test]
    async fn test_heuristic_policy() {
        let policy = HeuristicPolicy::new()
            .with_name("テストヒューリスティックポリシー")
            .add_rule(TestRule {
                name: "初期状態ルール".to_string(),
                target_state: TestState::Processing,
                output_event: TestEvent::Start,
            })
            .add_rule(TestRule {
                name: "処理状態ルール".to_string(),
                target_state: TestState::Final,
                output_event: TestEvent::Finish,
            });

        let current_state = TestState::Initial;
        let goal_state = Some(TestState::Processing);
        let observations: Vec<Observation<TestState, TestEvent>> = Vec::new();
        let insights: Vec<Insight> = Vec::new();

        let decision = policy
            .decide(&current_state, goal_state.as_ref(), &observations, &insights)
            .await
            .unwrap();

        assert_eq!(decision.event, TestEvent::Start);
    }
} 