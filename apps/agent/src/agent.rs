use crate::{
    decision::{Decision, DecisionContext},
    episode::Episode,
    error::{self, AgentError, AgentError::PolicyError, Result as AgentResult},
    feedback::{Feedback, FeedbackType},
    goal::Goal,
    insight::Insight,
    observation::Observation,
    policy::Policy,
    storage::{MemoryStorage, Storage},
};
use async_trait::async_trait;
use futures_util::TryFutureExt;
use rustate::{
    integration::{SharedContext, SharedMachineRef},
    machine::{Machine, MachineBuilder},
    Context,
    StateTrait, EventTrait, IntoEvent,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
    marker::PhantomData,
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::Mutex;
use uuid::Uuid;

// Define AgentId type alias
pub type AgentId = String; // Or Uuid::Uuid if appropriate

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
    /// 共有コンテキストを使用するかどうか
    pub use_shared_context: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "汎用エージェント".to_string(),
            description: "状態機械に基づく汎用エージェント".to_string(),
            max_observations: Some(100),
            auto_record_observations: true,
            auto_generate_insights: true,
            use_shared_context: false,
        }
    }
}

/// 状態機械に基づく知的エージェント
pub struct Agent<S, E, SM, P>
where
    S: RuStateTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + Default
        + PartialEq
        + 'static
        + From<String>,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + IntoEvent
        + Default
        + Hash
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
{
    /// エージェントの一意ID
    pub id: String,
    /// エージェントの状態機械（共有参照または所有）
    machine_ref: Option<SharedMachineRef>,
    machine: Option<Machine<S, E>>,
    /// エージェントの設定
    pub config: AgentConfig,
    /// エージェントの決定ポリシー
    policy: Arc<P>,
    /// エージェントのストレージ
    storage: Arc<SM>,
    /// 現在のエピソード（ある場合）
    current_episode: Option<Episode<S, E>>,
    /// 共有コンテキスト（設定されている場合）
    shared_context: Option<Arc<Mutex<Context>>>,
    /// 型パラメータのマーカー
    _phantom: PhantomData<(S, E)>,
}

impl<S, E, SM, P> Agent<S, E, SM, P>
where
    S: RuStateTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + Default
        + PartialEq
        + 'static
        + From<String>,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + IntoEvent
        + Default
        + Hash
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
{
    /// 新しいエージェントを作成します
    pub fn new(
        id: impl Into<String>,
        machine_builder: MachineBuilder<(), E, S, S>,
        policy: P,
        storage: SM,
        config: Option<AgentConfig>,
        shared_context: Option<Arc<Mutex<Context>>>,
    ) -> AgentResult<Self> {
        let machine = machine_builder
            .build()
            .map_err(|e| AgentError::StateMachineError(format!("Machine build failed: {}", e)))?;

        let final_config = config.unwrap_or_default();
        let final_shared_context = if final_config.use_shared_context {
            shared_context.or_else(|| Some(Arc::new(Mutex::new(Context::default()))))
        } else {
            None
        };

        Ok(Self {
            id: id.into(),
            machine_ref: None,
            machine: Some(machine),
            config: final_config,
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            shared_context: final_shared_context,
            _phantom: PhantomData,
        })
    }

    /// 共有状態機械参照を使用してエージェントを作成します
    pub fn with_shared_machine(
        id: impl Into<String>,
        machine_ref: SharedMachineRef,
        policy: P,
        storage: SM,
        config: Option<AgentConfig>,
    ) -> AgentResult<Self> {
        let final_config = config.unwrap_or_default();
        let final_shared_context = if final_config.use_shared_context {
            Some(Arc::new(Mutex::new(Context::default())))
        } else {
            None
        };

        Ok(Self {
            id: id.into(),
            machine_ref: Some(machine_ref),
            machine: None,
            config: final_config,
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            shared_context: final_shared_context,
            _phantom: PhantomData,
        })
    }

    /// エージェントの設定を変更します
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        if config.use_shared_context && self.shared_context.is_none() {
            self.shared_context = Some(Arc::new(Mutex::new(Context::default())));
        }
        self.config = config;
        self
    }

    /// 共有コンテキストを追加します
    pub fn with_shared_context(mut self, context: Arc<Mutex<Context>>) -> Self {
        self.shared_context = Some(context);
        self.config.use_shared_context = true;
        self
    }

    /// 現在の状態機械を取得します (Fails if using SharedMachineRef)
    pub fn machine(&self) -> AgentResult<&Machine<S, E>> {
        self.machine.as_ref().ok_or(AgentError::NotInitialized)
    }

    /// 現在の状態を取得します (May fail if using SharedMachineRef)
    pub fn current_state(&self) -> AgentResult<S> {
        self.machine().map(|m| m.state().clone())
    }

    /// Start a new episode for the agent.
    pub async fn start_episode<G: Into<Goal<S>>>(
        &mut self,
        name: impl Into<String>,
        initial_state: S,
        goal: G,
    ) -> AgentResult<()> {
        if self.current_episode.is_some() {
            return Err(AgentError::EpisodeAlreadyActive);
        }
        let goal_obj = goal.into();

        let episode = Episode::new(name.into(), initial_state.clone(), goal_obj);
        self.storage.save_episode(&episode).await?;
        self.current_episode = Some(episode);
        Ok(())
    }

    /// 現在のエピソードを完了させ、結果を返します
    pub async fn complete_episode(
        &mut self,
        is_successful: bool,
    ) -> AgentResult<Option<Episode<S, E>>> {
        if let Some(mut episode) = self.current_episode.take() {
            episode.complete(is_successful);
            self.storage.save_episode(&episode).await?;
            Ok(Some(episode))
        } else {
            Ok(None)
        }
    }

    /// エージェントに次の決定を行わせます
    pub async fn next_decision(&self) -> AgentResult<Decision<E>> {
        let current_state = self.current_state()?;
        let current_episode = self.current_episode().ok_or(AgentError::NoActiveEpisode)?;
        let goal_state = current_episode.goal.target_state.clone();

        let episode_id_str = current_episode.id.to_string();
        let observations = self.storage.get_observation(&episode_id_str).await?;
        let feedback = self.storage.get_feedback(&episode_id_str).await?;
        let insights = self.storage.get_insight(&episode_id_str).await?;

        let decision_context =
            DecisionContext::new(current_state, goal_state, observations, feedback, insights);

        self.make_decision(&decision_context).await
    }

    /// エージェントの次の状態遷移を実行します
    pub async fn step(&mut self) -> AgentResult<S> {
        if self.current_episode.is_none() {
            return Err(AgentError::NoActiveEpisode);
        }

        let current_state = self.current_state()?;
        if self.is_goal_reached(&current_state)? {
            return Err(AgentError::GoalReached);
        }

        let decision = self.next_decision().await?;
        let next_state = self.apply_decision(&decision).await?;

        if self.config.auto_record_observations {
            if let Some(episode) = self.current_episode.as_mut() {
                let observation =
                    Observation::new(current_state, decision.event.clone(), next_state.clone());
                episode.add_observation(observation.clone());
                self.storage
                    .save_observation(&episode.id.to_string(), &observation)
                    .await?;
            }
        }

        Ok(next_state)
    }

    /// ゴールに到達するか最大ステップ数に達するまでエージェントを実行します
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> AgentResult<bool> {
        let mut steps = 0;
        loop {
            if let Some(max) = max_steps {
                if steps >= max {
                    return Ok(false);
                }
            }

            match self.step().await {
                Ok(_) => {
                    steps += 1;
                }
                Err(AgentError::GoalReached) => {
                    self.complete_episode(true).await?;
                    return Ok(true);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    /// イベントを状態機械に適用し、新しい状態を返します
    pub async fn process_event(&mut self, event: E) -> AgentResult<S> {
        if let Some(ref _sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "Event processing via SharedMachineRef not implemented".to_string(),
            ))
        } else if let Some(machine) = self.machine.as_mut() {
            machine
                .send(event)
                .await
                .map(|state| state.clone())
                .map_err(AgentError::from)
        } else {
            Err(AgentError::NotInitialized)
        }
    }

    /// Adds an insight to the current episode and storage.
    pub async fn add_insight(&mut self, insight: Insight) -> AgentResult<()> {
        if let Some(episode) = self.current_episode.as_mut() {
            episode.add_insight(insight.clone());
            self.storage
                .save_insight(&episode.id.to_string(), &insight)
                .await?;
            Ok(())
        } else {
            Err(AgentError::NoActiveEpisode)
        }
    }

    /// Adds feedback to the current episode and storage.
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> AgentResult<()> {
        if let Some(episode) = self.current_episode.as_mut() {
            episode.add_feedback(feedback.clone());
            self.storage
                .save_feedback(&episode.id.to_string(), &feedback)
                .await?;
            Ok(())
        } else {
            Err(AgentError::NoActiveEpisode)
        }
    }

    /// 現在のエピソードを取得します
    pub fn current_episode(&self) -> Option<&Episode<S, E>> {
        self.current_episode.as_ref()
    }

    /// ポリシーを使用して決定を行います
    async fn make_decision(
        &self,
        decision_context: &DecisionContext<S, E>,
    ) -> AgentResult<Decision<E>> {
        self.policy
            .decide(decision_context)
            .await
            .map_err(Into::into)
    }

    /// Returns the policy object.
    pub fn policy(&self) -> Arc<P> {
        self.policy.clone()
    }

    /// Returns the storage object.
    pub fn storage(&self) -> Arc<SM> {
        self.storage.clone()
    }

    /// Internal method to process feedback.
    pub async fn process_feedback(&self, _feedback: &Feedback<E>) -> AgentResult<()> {
        Ok(())
    }

    /// Applies the chosen decision to the state machine.
    async fn apply_decision(&mut self, decision: &Decision<E>) -> AgentResult<S> {
        self.process_event(decision.event.clone()).await
    }

    /// ゴール状態に到達したかどうかを確認します
    fn is_goal_reached(&self, current_state: &S) -> AgentResult<bool> {
        if let Some(episode) = self.current_episode.as_ref() {
            Ok(current_state == &episode.goal.target_state)
        } else {
            Err(AgentError::NoActiveEpisode)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decision::{Decision, DecisionContext},
        episode::Episode,
        error::{AgentError, PolicyError, Result as AgentResult},
        feedback::{Feedback, FeedbackType},
        goal::Goal,
        insight::{Insight, InsightType},
        observation::Observation,
        policy::Policy,
        storage::{MemoryStorage, Storage},
    };
    use async_trait::async_trait;
    use rustate::{
        Context, Event, EventTrait, IntoEvent, Machine, MachineBuilder, SharedMachineRef, State,
        StateTrait, Transition,
    };
    use serde::{Deserialize, Serialize};
    use std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::SystemTime,
    };
    use tokio::sync::Mutex;
    use uuid::Uuid;

    // Mock implementations for testing
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
    enum TestState {
        #[default]
        Idle,
        Running,
        Stopped,
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

    impl From<String> for TestState {
        fn from(s: String) -> Self {
            match s.as_str() {
                "Idle" => TestState::Idle,
                "Running" => TestState::Running,
                "Stopped" => TestState::Stopped,
                _ => TestState::Idle, // Default fallback
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
    enum TestEvent {
        #[default]
        Start,
        Stop,
        NoOp,
    }

    impl Display for TestEvent {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "START",
                TestEvent::Stop => "STOP",
                TestEvent::NoOp => "NOOP",
            }
        }

        fn payload(&self) -> Option<&serde_json::Value> {
            None
        }

        fn name(&self) -> &str {
            match self {
                TestEvent::Start => "Start",
                TestEvent::Stop => "Stop",
                TestEvent::NoOp => "NoOp",
            }
        }
    }

    impl IntoEvent for TestEvent {
        fn into_event(self) -> Event {
            Event::new(self.event_type())
        }
    }

    struct MockPolicy;

    #[async_trait]
    impl Policy<TestState, TestEvent> for MockPolicy {
        fn name(&self) -> &str {
            "MockPolicy"
        }
        fn description(&self) -> &str {
            "A mock policy for testing"
        }

        async fn decide(
            &self,
            context: &DecisionContext<TestState, TestEvent>,
        ) -> Result<Decision<TestEvent>, PolicyError> {
            let event = match context.current_state {
                TestState::Idle => TestEvent::Start,
                TestState::Running => TestEvent::Stop,
                TestState::Stopped => TestEvent::NoOp,
            };
            Ok(Decision::new(
                Uuid::new_v4().to_string(),
                event,
                1.0,
                Some("Mock decision".to_string()),
                Some(context.current_state.clone()),
            ))
        }
    }

    fn create_test_machine_builder() -> MachineBuilder<(), TestEvent, TestState, TestState> {
        let idle = State::new("Idle", TestState::Idle);
        let running = State::new("Running", TestState::Running);
        let stopped = State::new("Stopped", TestState::Stopped);

        MachineBuilder::new("test_machine", TestState::Idle)
            .context(())
            .state(idle)
            .state(running)
            .state(stopped)
            .transition(Transition::new("Idle", "START", "Running"))
            .transition(Transition::new("Running", "STOP", "Stopped"))
    }

    type MockStorage = crate::storage::MemoryStorage<TestState, TestEvent>;
    type TestAgent = Agent<TestState, TestEvent, MockStorage, MockPolicy>;

    // Helper function to create a test agent
    fn create_test_agent() -> TestAgent {
        let machine_builder = create_test_machine_builder();
        let policy = MockPolicy;
        let storage = MockStorage::new();
        Agent::new("test_agent", machine_builder, policy, storage, None, None).unwrap()
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = create_test_agent();
        assert_eq!(agent.id, "test_agent");
        assert!(agent.machine.is_some());
        assert!(agent.current_episode.is_none());
    }

    #[tokio::test]
    async fn test_agent_run_step() {
        let mut agent = create_test_agent();
        agent
            .start_episode(
                "test_episode",
                TestState::Idle,
                Goal::new(TestState::Stopped),
            )
            .await
            .unwrap();

        let next_state = agent.step().await.unwrap();
        assert_eq!(next_state, TestState::Running);
        let current_state = agent.current_state().unwrap();
        assert_eq!(current_state, TestState::Running);
        assert!(agent.current_episode.is_some());
        let episode = agent.current_episode().unwrap();
        assert_eq!(episode.observations.len(), 1);
        assert_eq!(episode.observations[0].previous_state, TestState::Idle);
        assert_eq!(episode.observations[0].event, TestEvent::Start);
        assert_eq!(episode.observations[0].next_state, TestState::Running);
    }

    #[tokio::test]
    async fn test_agent_process_feedback() {
        let agent = create_test_agent();

        let feedback = Feedback::new(
            "Good job!".to_string(),
            FeedbackType::Positive,
            "user".to_string(),
        );

        let result = agent.process_feedback(&feedback).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_get_insights() {
        let mut agent = create_test_agent();
        agent
            .start_episode(
                "insight_test",
                TestState::Idle,
                Goal::new(TestState::Stopped),
            )
            .await
            .unwrap();

        let observation = Observation::new(TestState::Idle, TestEvent::Start, TestState::Running);
        let episode = Episode::new(
            agent.id.clone(),
            TestState::Idle,
            Goal::new(TestState::Stopped),
        );

        let insight1 = Insight::new(
            "insight_id_1".to_string(),
            "Insight 1".to_string(),
            InsightType::General,
        );
        let _ = agent.add_insight(insight1).await;

        let episode_id = agent.current_episode().unwrap().id.to_string();
        let stored_insights = agent.storage().get_insight(&episode_id).await.unwrap();
        assert_eq!(stored_insights.len(), 1);
        assert_eq!(stored_insights[0].content, "Insight 1");
    }
}
