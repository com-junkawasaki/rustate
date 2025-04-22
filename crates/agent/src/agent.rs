use crate::{
    decision::{Decision, DecisionContext},
    episode::Episode,
    error::{AgentError, Result},
    feedback::Feedback,
    insight::Insight,
    observation::Observation,
    policy::Policy,
    storage::Storage,
};
use rustate::{Context, EventTrait, Machine, StateTrait};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

/// エージェントの構成設定
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// エージェントの名前
    pub name: String,
    /// エージェントの説明
    pub description: String,
    /// 観測データの最大保持数（Noneの場合は無制限）
    pub max_observations: Option<usize>,
    /// イベント処理時に自動的に観測データを記録するかどうか
    pub auto_record_observations: bool,
    /// 状態遷移時に自動的に洞察を生成するかどうか
    pub auto_generate_insights: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "汎用エージェント".to_string(),
            description: "状態機械に基づく汎用エージェント".to_string(),
            max_observations: Some(100),
            auto_record_observations: true,
            auto_generate_insights: true,
        }
    }
}

/// 状態機械に基づく知的エージェント
pub struct Agent<S, E, SM, P>
where
    S: StateTrait + Clone + Debug + DeserializeOwned + Send + Sync + PartialEq + 'static + Default,
    E: EventTrait + Clone + Debug + DeserializeOwned + Send + Sync + 'static + rustate::IntoEvent,
    SM: Storage<S, E>,
    P: Policy<S, E>,
{
    /// エージェントの状態機械
    pub machine: Machine<S, E>,
    /// エージェントの設定
    pub config: AgentConfig,
    /// エージェントの決定ポリシー
    policy: Arc<P>,
    /// エージェントのストレージ
    storage: Arc<SM>,
    /// 現在のエピソード（ある場合）
    current_episode: Option<Episode<S, E>>,
    /// 型パラメータのマーカー
    _phantom: PhantomData<(S, E)>,
}

impl<S, E, SM, P> Agent<S, E, SM, P>
where
    S: StateTrait + DeserializeOwned + Debug + Clone + Send + Sync + PartialEq + 'static + Default,
    E: EventTrait + DeserializeOwned + Debug + Clone + Send + Sync + 'static + rustate::IntoEvent,
    SM: Storage<S, E>,
    P: Policy<S, E>,
{
    /// 新しいエージェントを作成します
    pub fn new(machine: Machine<S, E>, policy: P, storage: SM) -> Self {
        Self {
            machine,
            config: AgentConfig::default(),
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            _phantom: PhantomData,
        }
    }

    /// エージェントの設定を変更します
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// 新しいエピソードを開始します
    pub async fn start_episode(
        &mut self,
        name: impl Into<String>,
        goal_state: Option<S>,
    ) -> Result<(), AgentError> {
        // 初期状態を取得
        let initial_state = self.machine.current_state().clone();

        // 目標状態が指定されていない場合はエラー
        let goal = match goal_state {
            Some(state) => state,
            None => {
                return Err(AgentError::Other(
                    "目標状態が設定されていません".to_string(),
                ))
            }
        };

        // 新しいエピソードを作成
        let episode = Episode::new(name.into(), initial_state, goal);

        // エピソードを保存
        self.storage.save_episode(&episode).await?;

        // 現在のエピソードを設定
        self.current_episode = Some(episode);

        Ok(())
    }

    /// 現在のエピソードを完了します
    pub async fn complete_episode(
        &mut self,
        is_successful: bool,
    ) -> Result<Option<Episode<S, E>>, AgentError> {
        if let Some(mut episode) = self.current_episode.take() {
            episode.complete(is_successful);
            self.storage.save_episode(&episode).await?;
            return Ok(Some(episode));
        }
        Ok(None)
    }

    /// 次の決定を生成します
    pub async fn next_decision(&self) -> Result<Decision<E>, AgentError> {
        // 現在のエピソードがなければエラー
        if self.current_episode.is_none() {
            return Err(AgentError::Other(
                "エピソードが開始されていません".to_string(),
            ));
        }

        // make_decision メソッドを使用して次の決定を取得
        self.make_decision().await
    }

    /// 決定に基づいてイベントを適用します
    pub async fn apply_decision(&mut self, decision: &Decision<E>) -> Result<S, AgentError> {
        let previous_state = self.machine.current_state().clone();

        // イベントを適用
        match self
            .machine
            .transition(decision.event.clone(), Context::default())
        {
            Ok(next_state) => {
                // 自動観測記録が有効な場合
                if self.config.auto_record_observations {
                    let observation = Observation::new(
                        previous_state.clone(),
                        decision.event.clone(),
                        next_state.clone(),
                    )
                    .with_metadata("decision_id", &decision.id);

                    self.storage.save_observation(&observation).await?;

                    // エピソードに観測を追加
                    if let Some(episode) = &mut self.current_episode {
                        episode.add_observation(observation);
                    }
                }

                // 自動洞察生成が有効な場合
                if self.config.auto_generate_insights {
                    // ここでは簡単な洞察生成の例を示します
                    // 実際の実装ではより高度な洞察生成ロジックが必要です
                    if previous_state != next_state {
                        let insight = Insight::new(
                            "状態遷移",
                            format!(
                                "{:?}から{:?}への遷移が観測されました",
                                previous_state, next_state
                            ),
                            0.9,
                        );

                        self.storage.save_insight(&insight).await?;

                        // エピソードに洞察を追加
                        if let Some(episode) = &mut self.current_episode {
                            episode.add_insight(insight);
                        }
                    }
                }

                Ok(next_state)
            }
            Err(e) => Err(AgentError::MachineError(e)),
        }
    }

    /// エージェントの自律的な実行ステップを1回実行します
    pub async fn step(&mut self) -> Result<S, AgentError> {
        let decision = self.next_decision().await?;
        self.apply_decision(&decision).await
    }

    /// エージェントを目標状態に到達するまで実行します
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> Result<bool, AgentError> {
        // 現在のエピソードを確認
        let episode = match &self.current_episode {
            Some(ep) => ep,
            None => return Err(AgentError::NoActiveEpisode),
        };

        // ゴール状態を取得
        let goal_state = episode.goal_state.clone();

        // 最大ステップ数または無制限にステップを実行
        let mut steps = 0;
        while max_steps.is_none_or(|max| steps < max) {
            // 現在の状態がゴール状態と一致しているか確認
            if self.machine.current_state() == goal_state {
                // ゴールに到達、エピソードを成功として完了
                self.complete_episode(true).await?;
                return Ok(true);
            }

            // 次のステップを実行
            let result = self.step().await;

            match result {
                Ok(_) => {
                    steps += 1;
                }
                Err(err) => {
                    // エピソードを失敗としてマーク
                    if let Some(episode) = &mut self.current_episode {
                        episode.complete(false);
                        self.storage.save_episode(episode).await?;
                    }
                    return Err(err);
                }
            }
        }

        // 最大ステップ数に達した場合は失敗とする
        if let Some(episode) = &mut self.current_episode {
            episode.complete(false);
            self.storage.save_episode(episode).await?;
        }

        Ok(false)
    }

    /// 新しい洞察を追加します
    pub async fn add_insight(&mut self, insight: Insight) -> Result<(), AgentError> {
        self.storage.save_insight(&insight).await?;

        // エピソードに洞察を追加
        if let Some(episode) = &mut self.current_episode {
            episode.add_insight(insight);
        }

        Ok(())
    }

    /// 新しいフィードバックを追加します
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> Result<(), AgentError> {
        self.storage.save_feedback(&feedback).await?;

        // エピソードにフィードバックを追加
        if let Some(episode) = &mut self.current_episode {
            episode.add_feedback(feedback);
        }

        Ok(())
    }

    /// 現在のエピソードを取得します
    pub fn current_episode(&self) -> Option<Episode<S, E>> {
        self.current_episode.clone()
    }

    /// 現在の状態に基づいて決定を行います
    pub async fn make_decision(&self) -> Result<Decision<E>, AgentError> {
        let state = self.machine.current_state();
        let goal_state = self
            .current_episode
            .as_ref()
            .map(|ep| ep.goal_state.clone());
        let observations = self.storage.find_observations(None, None).await?;
        let insights = self.storage.find_insights(None, None).await?;

        // 現在の状態を基に新しい決定を生成
        let context = DecisionContext::new(state.clone(), goal_state, &observations, &insights);

        let decision = self.policy.decide(context);

        // 決定を保存
        self.storage.save_decision(&decision).await?;
        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::Decision;
    use crate::episode::Episode;
    use crate::error::AgentError;
    use crate::feedback::Feedback;
    use crate::insight::Insight;
    use crate::observation::Observation;
    use crate::policy::Policy;
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum TestState {
        Idle,
        Processing,
        Completed,
        Error,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum TestAction {
        Start,
        Process,
        Complete,
        Retry,
        Abort,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestObservation {
        state: TestState,
        data: String,
    }

    impl Observation for TestObservation {
        type State = TestState;
        
        fn state(&self) -> &Self::State {
            &self.state
        }
        
        fn timestamp() -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }

    #[derive(Debug, Clone)]
    struct TestDecision {
        action: TestAction,
        confidence: f64,
    }

    impl Decision for TestDecision {
        type Action = TestAction;
        
        fn action(&self) -> &Self::Action {
            &self.action
        }
        
        fn confidence(&self) -> f64 {
            self.confidence
        }
        
        fn timestamp() -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }

    #[derive(Debug, Clone)]
    struct TestPolicy {
        state_action_map: HashMap<TestState, TestAction>,
        fallback_action: TestAction,
    }

    impl Policy for TestPolicy {
        type State = TestState;
        type Action = TestAction;
        type Observation = TestObservation;
        type Decision = TestDecision;
        
        fn decide(&self, observation: &Self::Observation) -> Self::Decision {
            let action = self.state_action_map
                .get(observation.state())
                .cloned()
                .unwrap_or_else(|| self.fallback_action.clone());
                
            TestDecision {
                action,
                confidence: 0.9,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct TestFeedback {
        success: bool,
        score: f64,
        message: String,
    }

    impl Feedback for TestFeedback {
        fn success(&self) -> bool {
            self.success
        }
        
        fn score(&self) -> f64 {
            self.score
        }
        
        fn timestamp() -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }

    #[derive(Debug, Clone)]
    struct TestInsight {
        key: String,
        value: String,
    }

    impl Insight for TestInsight {
        fn key(&self) -> &str {
            &self.key
        }
        
        fn value(&self) -> &str {
            &self.value
        }
        
        fn timestamp() -> chrono::DateTime<chrono::Utc> {
            chrono::Utc::now()
        }
    }

    // ポリシーの作成ヘルパー関数
    fn create_test_policy() -> TestPolicy {
        let mut state_action_map = HashMap::new();
        state_action_map.insert(TestState::Idle, TestAction::Start);
        state_action_map.insert(TestState::Processing, TestAction::Process);
        state_action_map.insert(TestState::Completed, TestAction::Complete);
        state_action_map.insert(TestState::Error, TestAction::Retry);
        
        TestPolicy {
            state_action_map,
            fallback_action: TestAction::Abort,
        }
    }

    #[test]
    fn test_agent_creation() {
        let policy = create_test_policy();
        let agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent",
            Box::new(policy),
        );
        
        assert_eq!(agent.name(), "test_agent");
        assert_eq!(agent.episodes().len(), 0);
    }

    #[test]
    fn test_agent_observe_and_decide() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent",
            Box::new(policy),
        );
        
        let observation = TestObservation {
            state: TestState::Idle,
            data: "初期状態".to_string(),
        };
        
        let decision = agent.observe_and_decide(observation.clone());
        
        // 期待される判断: Idleに対してはStartアクション
        assert_eq!(*decision.action(), TestAction::Start);
        
        // エピソードが作成されたことを確認
        assert_eq!(agent.episodes().len(), 1);
        
        // 最新のエピソードを取得
        let episode = agent.latest_episode().unwrap();
        
        // エピソードに観測と判断が記録されていることを確認
        assert_eq!(episode.observations().len(), 1);
        assert_eq!(episode.decisions().len(), 1);
        
        // 観測の状態を確認
        let recorded_observation = &episode.observations()[0];
        assert_eq!(recorded_observation.state(), &TestState::Idle);
    }

    #[test]
    fn test_agent_multiple_observations() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // 状態遷移シーケンス: Idle -> Processing -> Completed
        let observations = vec![
            TestObservation { state: TestState::Idle, data: "初期状態".to_string() },
            TestObservation { state: TestState::Processing, data: "処理中".to_string() },
            TestObservation { state: TestState::Completed, data: "完了".to_string() },
        ];
        
        // 各状態を観測し判断
        for observation in observations {
            let decision = agent.observe_and_decide(observation);
            assert!(matches!(decision.action(), 
                TestAction::Start | TestAction::Process | TestAction::Complete
            ));
        }
        
        // 一つのエピソードが存在し、3つの観測と判断が記録されていることを確認
        assert_eq!(agent.episodes().len(), 1);
        let episode = agent.latest_episode().unwrap();
        assert_eq!(episode.observations().len(), 3);
        assert_eq!(episode.decisions().len(), 3);
        
        // 状態の遷移順序を確認
        assert_eq!(episode.observations()[0].state(), &TestState::Idle);
        assert_eq!(episode.observations()[1].state(), &TestState::Processing);
        assert_eq!(episode.observations()[2].state(), &TestState::Completed);
    }

    #[test]
    fn test_agent_feedback() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // 観測と判断
        let observation = TestObservation {
            state: TestState::Idle,
            data: "初期状態".to_string(),
        };
        
        let decision = agent.observe_and_decide(observation);
        assert_eq!(*decision.action(), TestAction::Start);
        
        // フィードバックを提供
        let feedback = TestFeedback {
            success: true,
            score: 0.95,
            message: "良好な判断".to_string(),
        };
        
        agent.provide_feedback(feedback);
        
        // エピソードにフィードバックが記録されていることを確認
        let episode = agent.latest_episode().unwrap();
        assert_eq!(episode.feedbacks().len(), 1);
        assert!(episode.feedbacks()[0].success());
        assert_eq!(episode.feedbacks()[0].score(), 0.95);
    }

    #[test]
    fn test_agent_insights() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // 観測と判断
        let observation = TestObservation {
            state: TestState::Idle,
            data: "初期状態".to_string(),
        };
        
        agent.observe_and_decide(observation);
        
        // インサイトを追加
        let insight = TestInsight {
            key: "performance".to_string(),
            value: "高速に応答".to_string(),
        };
        
        agent.add_insight(insight);
        
        // エピソードにインサイトが記録されていることを確認
        let episode = agent.latest_episode().unwrap();
        assert_eq!(episode.insights().len(), 1);
        assert_eq!(episode.insights()[0].key(), "performance");
        assert_eq!(episode.insights()[0].value(), "高速に応答");
    }

    #[test]
    fn test_agent_new_episode() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // 最初のエピソード
        let observation1 = TestObservation {
            state: TestState::Idle,
            data: "エピソード1".to_string(),
        };
        
        agent.observe_and_decide(observation1);
        assert_eq!(agent.episodes().len(), 1);
        
        // 新しいエピソードを開始
        agent.start_new_episode();
        
        // 二つ目のエピソードの観測と判断
        let observation2 = TestObservation {
            state: TestState::Processing,
            data: "エピソード2".to_string(),
        };
        
        agent.observe_and_decide(observation2);
        
        // 2つのエピソードが存在することを確認
        assert_eq!(agent.episodes().len(), 2);
        
        // 最新のエピソードを確認
        let latest_episode = agent.latest_episode().unwrap();
        assert_eq!(latest_episode.observations().len(), 1);
        assert_eq!(latest_episode.observations()[0].state(), &TestState::Processing);
    }

    #[test]
    fn test_agent_episode_retrieval() {
        let policy = create_test_policy();
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // 3つのエピソードを生成
        for i in 0..3 {
            if i > 0 {
                agent.start_new_episode();
            }
            
            let state = match i {
                0 => TestState::Idle,
                1 => TestState::Processing,
                _ => TestState::Completed,
            };
            
            let observation = TestObservation {
                state,
                data: format!("エピソード{}", i + 1),
            };
            
            agent.observe_and_decide(observation);
        }
        
        // エピソード数を確認
        assert_eq!(agent.episodes().len(), 3);
        
        // インデックスでエピソードを取得
        let episode0 = agent.get_episode(0).unwrap();
        let episode1 = agent.get_episode(1).unwrap();
        let episode2 = agent.get_episode(2).unwrap();
        
        // 各エピソードの状態を確認
        assert_eq!(episode0.observations()[0].state(), &TestState::Idle);
        assert_eq!(episode1.observations()[0].state(), &TestState::Processing);
        assert_eq!(episode2.observations()[0].state(), &TestState::Completed);
        
        // 存在しないインデックスのエピソード取得を試みる
        let result = agent.get_episode(3);
        assert!(result.is_none());
    }

    #[test]
    fn test_agent_error_handling() {
        // エラー状態を処理するポリシーを作成
        let mut policy = create_test_policy();
        policy.state_action_map.insert(TestState::Error, TestAction::Retry);
        
        let mut agent = Agent::<TestState, TestAction, TestObservation, TestDecision, TestFeedback, TestInsight>::new(
            "test_agent", 
            Box::new(policy),
        );
        
        // エラー状態の観測
        let observation = TestObservation {
            state: TestState::Error,
            data: "エラー発生".to_string(),
        };
        
        // エージェントの判断を取得
        let decision = agent.observe_and_decide(observation);
        
        // エラー状態に対してRetryアクションが選択されることを確認
        assert_eq!(*decision.action(), TestAction::Retry);
        
        // 失敗フィードバックを提供
        let feedback = TestFeedback {
            success: false,
            score: 0.1,
            message: "エラー処理が必要".to_string(),
        };
        
        agent.provide_feedback(feedback);
        
        // エピソードのフィードバックを確認
        let episode = agent.latest_episode().unwrap();
        assert_eq!(episode.feedbacks().len(), 1);
        assert!(!episode.feedbacks()[0].success());
        assert_eq!(episode.feedbacks()[0].score(), 0.1);
    }
}
