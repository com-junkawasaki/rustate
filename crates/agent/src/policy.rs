use crate::{
    decision::Decision,
    error::AgentError,
    insight::Insight,
    observation::Observation,
};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rustate::{EventTrait, StateTrait};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::sync::Arc;

/// エージェントの決定ポリシーを定義するトレイト
#[async_trait]
pub trait Policy<S, E>
where
    S: StateTrait + DeserializeOwned + 'static,
    E: EventTrait + DeserializeOwned + 'static,
{
    /// ポリシーの名前を返します
    fn name(&self) -> &str {
        "基本ポリシー"
    }
    
    /// ポリシーの説明を返します
    fn description(&self) -> &str {
        "基本的な決定ポリシー"
    }
    
    /// 現在の状態とゴール状態から次のアクションを決定します
    async fn decide(
        &self,
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> std::result::Result<Decision<E>, AgentError>;
}

/// 利用可能なイベントからランダムに選択するシンプルなポリシー
pub struct RandomPolicy<E>
where
    E: EventTrait + Clone,
{
    available_events: Vec<E>,
    name: String,
    description: String,
}

impl<E> RandomPolicy<E>
where
    E: EventTrait + Clone,
{
    pub fn new(available_events: Vec<E>) -> Self {
        Self {
            available_events,
            name: "ランダムポリシー".to_string(),
            description: "利用可能なイベントからランダムに選択するポリシー".to_string(),
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
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
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
        _current_state: &S,
        _goal_state: Option<&S>,
        _observations: &[Observation<S, E>],
        _insights: &[Insight],
    ) -> std::result::Result<Decision<E>, AgentError> {
        let event = self.available_events
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| AgentError::PolicyError("利用可能なイベントがありません".to_string()))?;
        
        // ランダムな信頼度（0.5〜1.0）
        let confidence = 0.5 + rand::random::<f64>() * 0.5;
        
        Ok(Decision::new(event, confidence))
    }
}

/// ヒューリスティックルールによって決定を行うポリシー
pub struct HeuristicPolicy<S, E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    rules: Vec<Box<dyn HeuristicRule<S, E> + Send + Sync>>,
    fallback_policy: Box<dyn Policy<S, E> + Send + Sync>,
    name: String,
    description: String,
}

impl<S, E> HeuristicPolicy<S, E>
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
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
    S: StateTrait + DeserializeOwned + Debug + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + 'static,
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
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
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
        current_state: &S,
        goal_state: Option<&S>,
        observations: &[Observation<S, E>],
        insights: &[Insight],
    ) -> std::result::Result<Decision<E>, AgentError> {
        // マッチするルールを探す
        let mut rule_matches = Vec::new();
        
        for rule in &self.rules {
            if rule.matches(current_state, goal_state, observations, insights) {
                rule_matches.push(rule);
            }
        }
        
        if rule_matches.is_empty() {
            // マッチするルールがない場合はフォールバックポリシーを使用
            return self.fallback_policy.decide(current_state, goal_state, observations, insights).await;
        }
        
        // 優先度が最も高いルールを選択
        let best_rule = rule_matches.iter().max_by_key(|r| rule.priority()).unwrap();
        
        // 選択されたルールからイベントを取得
        let event = best_rule.get_event(current_state, goal_state);
        let confidence = best_rule.confidence();
        
        Ok(Decision::new(event, confidence))
    }
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

        fn priority(&self) -> i32 {
            1
        }

        fn matches(
            &self,
            current_state: &TestState,
            goal_state: Option<&TestState>,
            _observations: &[Observation<TestState, TestEvent>],
            _insights: &[Insight],
        ) -> bool {
            match goal_state {
                Some(goal) if *goal == self.target_state => true,
                _ => false,
            }
        }

        fn get_event(&self, _current_state: &TestState, _goal_state: Option<&TestState>) -> TestEvent {
            self.output_event.clone()
        }

        fn confidence(&self) -> f64 {
            1.0
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
        let policy = HeuristicPolicy::new(RandomPolicy::new(vec![TestEvent::Start]))
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
