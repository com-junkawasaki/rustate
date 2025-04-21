use crate::{
    decision::{Decision, DecisionContext},
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

/// エージェントの判断ポリシーを表すトレイト
pub trait Policy<S, E>
where
    S: StateTrait + Debug + Send + Sync + DeserializeOwned + 'static,
    E: EventTrait + Debug + Send + Sync + DeserializeOwned + Clone + 'static,
{
    /// ポリシーの名前を返します
    fn name(&self) -> &str {
        "基本ポリシー"
    }

    /// ポリシーの説明を返します
    fn description(&self) -> &str {
        "基本的な決定ポリシー"
    }

    /// 現在の状態と文脈に基づいて決定を行います
    fn decide(&self, context: DecisionContext<S, E>) -> Decision<E>;

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
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn decide(&self, _context: DecisionContext<S, E>) -> Decision<E> {
        let event = match self.available_events.choose(&mut rand::thread_rng()) {
            Some(e) => e.clone(),
            None => panic!("ランダムポリシーにイベントが設定されていません"),
        };

        // ランダムな信頼度（0.5〜1.0）
        let confidence = 0.5 + rand::random::<f64>() * 0.5;

        Decision::new(event, confidence)
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
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
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
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
    E: EventTrait + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn decide(&self, context: DecisionContext<S, E>) -> Decision<E> {
        // マッチするルールを探す
        let mut rule_matches = Vec::new();

        for rule in &self.rules {
            if rule.matches(
                &context.current_state,
                context.goal_state.as_ref(),
                context.observations,
                context.insights,
            ) {
                rule_matches.push(rule);
            }
        }

        if rule_matches.is_empty() {
            // マッチするルールがない場合はフォールバックポリシーを使用
            return self.fallback_policy.decide(context);
        }

        // 優先度が最も高いルールを選択
        let best_rule = rule_matches
            .iter()
            .max_by_key(|rule| rule.priority())
            .unwrap();

        // 選択されたルールからイベントを取得
        let event = best_rule.get_event(&context.current_state, context.goal_state.as_ref());
        let confidence = best_rule.confidence();

        Decision::new(event, confidence)
    }
}

/// Arcの中にPolicyトレイトを実装した型を格納するための型エイリアス
pub type PolicyBox<S, E> = Arc<dyn Policy<S, E>>;

#[cfg(test)]
mod tests {
    use super::*;
    
    
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
        let decision = policy.decide(DecisionContext::new(current_state, None, &[], &[]));

        assert!(matches!(
            decision.event,
            TestEvent::Start | TestEvent::Process | TestEvent::Finish
        ));
        assert!(decision.confidence >= 0.5 && decision.confidence <= 1.0);
    }

    // ... other tests ...
}
