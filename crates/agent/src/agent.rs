use crate::{
    decision::{Decision, DecisionContext, DecisionMaker},
    episode::Episode,
    error::AgentError,
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

        // ゴール状態の取得
        let goal_state = match &episode.goal_state {
            Some(goal) => goal.clone(),
            None => return Err(AgentError::NoGoalDefined),
        };

        // 最大ステップ数または無制限にステップを実行
        let mut steps = 0;
        while max_steps.map_or(true, |max| steps < max) {
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
    use crate::policy::RandomPolicy;
    use crate::storage::MemoryStorage;
    use rustate::{Event, EventTrait, IntoEvent, State, StateTrait, StateType, Transition};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl Default for TestState {
        fn default() -> Self {
            TestState::Initial
        }
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
            static NORMAL: StateType = StateType::Normal;
            &NORMAL
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

    impl IntoEvent for TestEvent {
        fn into_event(self) -> Event {
            Event::new(self.event_type())
        }
    }

    fn create_test_machine() -> Machine<TestState, TestEvent> {
        // 正しい実装に変更: StateオブジェクトとMachineBuilderを使用
        let initial_state = State::new("initial");
        let processing_state = State::new("processing");
        let final_state = State::new("final");
        
        let start_transition = Transition::new("initial", "start", "processing");
        let process_transition = Transition::new("processing", "process", "processing");
        let finish_transition = Transition::new("processing", "finish", "final");
        
        // 状態IDからTestStateへのマッパー関数
        let state_mapper = |state_id: &str| -> TestState {
            match state_id {
                "initial" => TestState::Initial,
                "processing" => TestState::Processing,
                "final" => TestState::Final,
                _ => panic!("不明な状態ID: {}", state_id),
            }
        };
        
        // ステートマシンを構築し、ステートマッパーを設定
        rustate::MachineBuilder::new("TestMachine")
            .state(initial_state)
            .state(processing_state)
            .state(final_state)
            .initial("initial")
            .transition(start_transition)
            .transition(process_transition)
            .transition(finish_transition)
            .build()
            .unwrap()
            .with_state_mapper(state_mapper)
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![
            TestEvent::Start,
            TestEvent::Process,
            TestEvent::Finish,
        ]);
        let storage = MemoryStorage::new();

        let agent = Agent::new(machine, policy, storage);
        assert_eq!(agent.config.name, "汎用エージェント");
        assert_eq!(agent.machine.current_state(), TestState::Initial);
    }

    #[tokio::test]
    async fn test_agent_episode() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![
            TestEvent::Start,
            TestEvent::Process,
            TestEvent::Finish,
        ]);
        let storage = MemoryStorage::new();

        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テストエピソード", Some(TestState::Final))
            .await
            .unwrap();

        assert!(agent.current_episode.is_some());
        let episode = agent.current_episode.as_ref().unwrap();
        assert_eq!(episode.name, "テストエピソード");
        assert_eq!(episode.initial_state, TestState::Initial);
        assert_eq!(&episode.goal_state, &TestState::Final);
    }

    #[tokio::test]
    async fn test_agent_decision_and_apply() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![TestEvent::Start]);
        let storage = MemoryStorage::new();

        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テスト決定適用", Some(TestState::Final))
            .await
            .unwrap();

        // 決定を取得して適用
        let decision = agent.next_decision().await.unwrap();
        assert_eq!(decision.event, TestEvent::Start);

        let next_state = agent.apply_decision(&decision).await.unwrap();
        assert_eq!(next_state, TestState::Processing);
        assert_eq!(agent.machine.current_state(), TestState::Processing);
    }

    #[tokio::test]
    async fn test_agent_step() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![TestEvent::Start]);
        let storage = MemoryStorage::new();

        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テストステップ", Some(TestState::Final))
            .await
            .unwrap();

        // ステップを実行
        let next_state = agent.step().await.unwrap();
        assert_eq!(next_state, TestState::Processing);
        assert_eq!(agent.machine.current_state(), TestState::Processing);
    }
}
