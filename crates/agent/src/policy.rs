use crate::{
    decision::{Decision, DecisionMaker},
    error::AgentError,
    insight::Insight,
    observation::Observation,
};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rustate::{StateTrait, EventTrait};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;

/// エージェントの決定ポリシーを定義するトレイト
#[async_trait]
pub trait Policy<S, E>: Send + Sync
where
    S: StateTrait + DeserializeOwned + 'static,
    E: EventTrait + DeserializeOwned + 'static,
{
    /// 名前を返します
    fn name(&self) -> &str {
        "汎用ポリシー"
    }
    
    /// 説明を返します
    fn description(&self) -> &str {
        "基本決定ポリシー"
    }

    /// 現在の状態と目標状態から次の決定を行います
    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> std::result::Result<Decision<E>, AgentError>;
}

/// ランダムポリシー - 利用可能なイベントからランダムに選択します。
/// テストと基本的なエージェント動作のデモに適しています。
pub struct RandomPolicy<E: EventTrait> {
    name: String,
    description: String,
    available_events: Vec<E>,
}

impl<E: EventTrait> RandomPolicy<E> {
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
    S: StateTrait + DeserializeOwned + Debug + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn decide(
        &self,
        _current_state: &S,
        _goal_state: Option<&S>,
        _observations: &[Observation<S, E>],
        _insights: &[Insight],
    ) -> std::result::Result<Decision<E>, AgentError> {
        let event = self.available_events
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| AgentError::DecisionError("利用可能なイベントがありません".to_string()))?;
        
        // ランダムな信頼度 (0.1〜0.5)
        let confidence = 0.1 + rand::random::<f64>() * 0.4;
        
        Ok(Decision::new(event, confidence))
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
    S: StateTrait,
    E: EventTrait,
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
    S: StateTrait + DeserializeOwned + Debug + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + 'static,
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
    ) -> std::result::Result<Decision<E>, AgentError> {
        // この実装は非常に単純なもので、実際のアプリケーションではより高度なロジックが必要です
        
        // ゴール状態が定義されていない場合は、ランダムな決定を返す
        if goal_state.is_none() {
            return Err(AgentError::DecisionError("目標状態が定義されていません".to_string()));
        }
        
        // 観測データからルールを適用
        let mut rule_matches = Vec::new();
        
        for rule in &self.rules {
            if rule.matches(current_state, goal_state, observations, insights) {
                rule_matches.push(rule);
            }
        }
        
        // 合致するルールがない場合はエラー
        if rule_matches.is_empty() {
            return Err(AgentError::DecisionError("適切なルールが見つかりません".to_string()));
        }
        
        // 最も優先度の高いルールを選択
        let best_rule = rule_matches.iter().max_by_key(|r| r.priority()).unwrap();
        
        // ルールから決定を作成
        let event = best_rule.get_event(current_state, goal_state)?;
        let confidence = best_rule.confidence();
        
        Ok(Decision::new(event, confidence))
    }
}

/// ヒューリスティックルールのトレイト
#[async_trait]
pub trait HeuristicRule<S, E>
where
    S: StateTrait,
    E: EventTrait,
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
} 
