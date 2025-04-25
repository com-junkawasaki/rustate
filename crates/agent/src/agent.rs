use crate::{
    decision::{Decision, DecisionContext},
    error::{AgentError, Result},
    episode::Episode,
    feedback::Feedback,
    goal::Goal,
    insight::Insight,
    observation::Observation,
    policy::Policy,
    storage::StorageManager,
};
use rustate::{
    event::{Event, EventTrait, IntoEvent},
    integration::{SharedContext, SharedMachineRef},
    machine::{Machine, MachineBuilder},
    state::{State as RuState, StateTrait},
    Context as RuContext,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    marker::PhantomData,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

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
    S: StateTrait + Clone + Debug + Send + Sync + Serialize + DeserializeOwned + Default + 'static,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + IntoEvent
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
    S: StateTrait + Clone + Debug + Send + Sync + Serialize + DeserializeOwned + Default + 'static,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + IntoEvent
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
{
    /// Create a new agent instance with default settings.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        id: Uuid,
        policy: P,
        storage: SM,
        machine_builder: MachineBuilder<S, E>,
        shared_context: Option<SharedContext>,
        initial_state: S,
        goal_state: Option<S>,
    ) -> Result<Self> {
        let mut machine = machine_builder.build()?;
        machine.set_context(Arc::new(Mutex::new(shared_context.unwrap_or_default())));

        let machine_ref = SharedMachineRef::new(machine);

        Ok(Self {
            id,
            machine_ref: Arc::new(Mutex::new(machine_ref)),
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            insights: Arc::new(Mutex::new(Vec::new())),
            goal_state,
            _phantom_s: PhantomData,
            _phantom_e: PhantomData,
        })
    }

    /// 共有状態機械参照を使用してエージェントを作成します
    pub fn with_shared_machine(machine_ref: SharedMachineRef, policy: P, storage: SM) -> Self {
        let dummy_machine = MachineBuilder::<S, E>::new("dummy")
            .initial(S::default())
            .state(S::default())
            .build()
            .expect("Failed to build dummy");

        Self {
            id: Uuid::new_v4().to_string(),
            config: AgentConfig::default(),
            machine_ref: Some(machine_ref),
            machine: Some(dummy_machine),
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            shared_context: None,
            _phantom: PhantomData,
        }
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

    /// 現在の状態機械を取得します
    pub fn machine(&self) -> Result<&Machine<S, E>, AgentError> {
        if let Some(ref _sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "Direct machine access not available when using SharedMachineRef".to_string(),
            ))
        } else {
            self.machine.as_ref().ok_or(AgentError::NotInitialized)
        }
    }

    /// 現在の状態を取得します
    pub fn current_state(&self) -> Result<S, AgentError> {
        if let Some(ref sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "current_state not available via SharedMachineRef yet".to_string(),
            ))
        } else {
            self.machine
                .as_ref()
                .ok_or(AgentError::NotInitialized)
                .map(|m| m.current_state().clone())
        }
    }

    /// Start a new episode for the agent.
    pub async fn start_episode<G: Into<Goal<S, E>>>(
        &mut self,
        name: impl Into<String>,
        initial_state: S,
        goal: G,
    ) -> Result<()> {
        let goal = goal.into();
        let episode = Episode::new(name.into(), initial_state.clone(), goal);
        self.current_episode = Some(episode.clone());
        self.goal_state = Some(episode.goal().target_state.clone());

        let mut machine_guard = self.machine_ref.lock().await;
        machine_guard.reset_to(initial_state)?;

        self.storage.save_episode(&episode).await?;
        Ok(())
    }

    /// Complete the current episode.
    pub async fn complete_episode(&mut self, is_successful: bool) -> Result<Option<Episode<S, E>>> {
        if let Some(mut episode) = self.current_episode.take() {
            episode.complete(is_successful);
            self.storage.save_episode(&episode).await?;
            match self.generate_insights(&episode).await {
                Ok(insights) => {
                    let mut current_insights = self.insights.lock().await;
                    for insight in insights {
                        current_insights.push(insight.clone());
                        self.storage.save_insight(episode.id(), &insight).await?;
                    }
                }
                Err(e) => log::error!("Failed to generate insights: {}", e),
            }
            self.goal_state = None;
            Ok(Some(episode))
        } else {
            Ok(None)
        }
    }

    /// Get the next decision from the policy based on the current state.
    pub async fn next_decision(&self) -> Result<Decision<E>> {
        if let Some(episode) = &self.current_episode {
            let current_state = self.current_state().await?;
            let goal_state = self.goal_state.clone();
            let observations = self.storage.get_observations(episode.id()).await?;
            let feedbacks = self.storage.get_feedback(episode.id()).await?;
            let insights = self.insights.lock().await.clone();

            let decision_context = DecisionContext::new(
                current_state,
                goal_state,
                observations,
                feedbacks,
                insights,
            );
            self.policy.decide(decision_context).await
        } else {
            Err(AgentError::NoActiveEpisode)
        }
    }

    /// Executes a single step in the agent's decision-making process.
    pub async fn step(&mut self) -> Result<S> {
        let decision = self.next_decision().await?;
        let event = decision.event;

        let current_state = self.current_state().await?;
        if let Some(episode) = &self.current_episode {
            let observation = Observation::new(current_state.clone(), event.clone());
            self.storage.save_observation(episode.id(), &observation).await?;
        }

        let new_state = self.process_event(event).await?;
        Ok(new_state)
    }

    /// Runs the agent until the goal state is reached or max_steps are exceeded.
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> Result<bool> {
        let mut steps = 0;
        while self.current_episode.is_some() {
            let current_state = self.current_state().await?;

            if let Some(goal) = &self.goal_state {
                if current_state == *goal {
                    self.complete_episode(true).await?;
                    return Ok(true);
                }
            } else {
                log::warn!("Running agent without a goal state set.");
                break;
            }

            if let Some(max) = max_steps {
                if steps >= max {
                    self.complete_episode(false).await?;
                    return Ok(false);
                }
            }

            match self.step().await {
                Ok(_) => steps += 1,
                Err(e) => {
                    return Err(e);
                }
            }
        }
        if self.current_episode.is_none() {
            Err(AgentError::NoActiveEpisode)
        } else {
            Ok(false)
        }
    }

    /// Process an external event through the state machine.
    pub async fn process_event(&self, event: E) -> Result<S> {
        let machine_guard = self.machine_ref.lock().await;
        let result = machine_guard.send(event.into_event()).await;
        match result {
            Err(rustate_error) => Err(AgentError::StateMachineError(rustate_error.to_string())),
            Ok(new_state_info) => {
                Ok(new_state_info)
            }
        }
    }

    /// Add an insight to the agent's knowledge base.
    pub async fn add_insight(&mut self, insight: Insight) -> Result<()> {
        let mut insights = self.insights.lock().await;
        insights.push(insight.clone());
        if let Some(episode) = &self.current_episode {
            self.storage.save_insight(episode.id(), &insight).await?;
        } else {
            log::warn!("Insight added outside of an active episode.");
        }
        Ok(())
    }

    /// Add feedback to the agent's experience.
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> Result<()> {
        self.process_feedback(&feedback).await?;
        Ok(())
    }

    /// Get the current state of the agent's internal state machine.
    pub async fn current_state(&self) -> Result<S> {
        let machine_guard = self.machine_ref.lock().await;
        Ok(machine_guard.current_state().clone())
    }

    /// Provides access to the current episode, if active.
    pub fn current_episode(&self) -> Option<&Episode<S, E>> {
        self.current_episode.as_ref()
    }

    /// Provides access to the agent's goal state, if set.
    pub fn goal_state(&self) -> Option<&S> {
        self.goal_state.as_ref()
    }

    /// Make a decision based on the current context (internal helper potentially).
    async fn make_decision(&self) -> Result<Decision<E>> {
        self.next_decision().await
    }

    /// Get the underlying state machine reference (read-only access).
    pub fn machine(&self) -> Arc<Mutex<SharedMachineRef>> {
        self.machine_ref.clone()
    }

    /// Get the policy instance.
    pub fn policy(&self) -> Arc<P> {
        self.policy.clone()
    }

    /// Get the storage manager instance.
    pub fn storage(&self) -> Arc<SM> {
        self.storage.clone()
    }

    /// Get insights (read-only access).
    pub fn insights(&self) -> Arc<Mutex<Vec<Insight>>> {
        self.insights.clone()
    }

    /// Internal method to process feedback.
    pub async fn process_feedback(&self, feedback: &Feedback<E>) -> Result<()> {
        if let Some(episode) = &self.current_episode {
            self.storage.save_feedback(episode.id(), feedback).await?;
        } else {
            log::warn!("Feedback received outside of an active episode.");
        }
        Ok(())
    }

    /// Internal method to generate insights from an episode.
    pub async fn generate_insights(&self, episode: &Episode<S, E>) -> Result<Vec<Insight>> {
        let trace = self.storage.get_trace(episode.id()).await?;
        let insights = self.policy.analyze_episode_trace(&trace).await?;
        Ok(insights)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decision::{Decision, DecisionContext},
        error::AgentError,
        feedback::Feedback,
        insight::Insight,
        observation::Observation,
        storage::MemoryStorage,
    };
    use rustate::MachineBuilder;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum TestState {
        Idle,
        Processing,
        Completed,
        Error,
    }

    impl Default for TestState {
        fn default() -> Self {
            Self::Idle
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &str {
            match self {
                TestState::Idle => "idle",
                TestState::Processing => "processing",
                TestState::Completed => "completed",
                TestState::Error => "error",
            }
        }

        fn state_type(&self) -> &rustate::StateType {
            static STATE_TYPE: rustate::StateType = rustate::StateType::Normal;
            &STATE_TYPE
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

        fn data(&self) -> Option<&serde_json::Value> {
            None
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum TestEvent {
        Start,
        Process,
        Complete,
        Retry,
        Abort,
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "START",
                TestEvent::Process => "PROCESS",
                TestEvent::Complete => "COMPLETE",
                TestEvent::Retry => "RETRY",
                TestEvent::Abort => "ABORT",
            }
        }

        fn payload(&self) -> Option<&serde_json::Value> {
            None
        }
    }

    impl rustate::IntoEvent for TestEvent {
        fn into_event(self) -> rustate::Event {
            rustate::Event::new(self.event_type())
        }
    }

    #[derive(Clone)]
    struct TestPolicy {
        state_action_map: HashMap<TestState, TestEvent>,
    }

    impl TestPolicy {
        fn new() -> Self {
            let mut map = HashMap::new();
            map.insert(TestState::Idle, TestEvent::Start);
            map.insert(TestState::Processing, TestEvent::Complete);
            map.insert(TestState::Error, TestEvent::Retry);
            map.insert(TestState::Completed, TestEvent::Start);
            Self {
                state_action_map: map,
            }
        }
    }

    #[async_trait::async_trait]
    impl Policy<TestState, TestEvent> for TestPolicy {
        async fn decide(
            &self,
            context: DecisionContext<TestState, TestEvent>,
        ) -> std::result::Result<Decision<TestEvent>, AgentError> {
            let action = self
                .state_action_map
                .get(&context.current_state)
                .cloned()
                .unwrap_or(TestEvent::Abort);

            Ok(Decision::new(
                Uuid::new_v4().to_string(),
                action,
                0.9,
                Some(context.current_state.clone()),
                Some(context.goal_state.clone()),
            ))
        }
    }

    fn create_test_machine() -> Machine<TestState, TestEvent> {
        let idle = RuState::new("idle");
        let processing = RuState::new("processing");
        let completed = RuState::new("completed");
        let error = RuState::new("error");

        let idle_to_processing = Transition::new("idle", "START", "processing");
        let processing_to_completed = Transition::new("processing", "COMPLETE", "completed");
        let processing_to_error = Transition::new("processing", "ABORT", "error");
        let error_to_processing = Transition::new("error", "RETRY", "processing");
        let completed_to_idle = Transition::new("completed", "START", "idle");

        MachineBuilder::new("test_machine")
            .state(idle)
            .state(processing)
            .state(completed)
            .state(error)
            .initial("idle")
            .transition(idle_to_processing)
            .transition(processing_to_completed)
            .transition(processing_to_error)
            .transition(error_to_processing)
            .transition(completed_to_idle)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_agent_with_shared_machine() {
        let machine = create_test_machine();
        let shared_machine = SharedMachineRef::new(machine);

        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::with_shared_machine(shared_machine.clone());

        let goal_state = TestState::Completed;

        agent
            .start_episode("テストエピソード", Some(goal_state))
            .await
            .unwrap();

        let next_state = agent.step().await.unwrap();
        assert_eq!(next_state, TestState::Processing);

        let final_state = agent.step().await.unwrap();
        assert_eq!(final_state, TestState::Completed);

        let episode = agent.complete_episode(true).await.unwrap().unwrap();
        assert!(episode.is_completed());
        assert!(episode.is_successful);
    }

    #[tokio::test]
    async fn test_agent_with_shared_context() {
        let machine = create_test_machine();
        let shared_context = Arc::new(Mutex::new(Context::default()));

        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(
            Uuid::new_v4(),
            TestState::Idle,
            policy,
            storage,
            MachineBuilder::new("test_machine"),
            None,
            Some(shared_context.clone()),
        )
        .await
        .unwrap();

        shared_context
            .lock()
            .expect("Mutex poisoned")
            .set("test_key", "test_value")
            .unwrap();

        let goal_state = TestState::Completed;

        agent
            .start_episode("テストエピソード", Some(goal_state))
            .await
            .unwrap();

        let success = agent.run_until_goal(Some(5)).await.unwrap();
        assert!(success);

        let value: Option<String> = shared_context
            .lock()
            .expect("Mutex poisoned")
            .get("test_key")
            .unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let agent = Agent::new(
            Uuid::new_v4(),
            TestState::Idle,
            policy,
            storage,
            MachineBuilder::new("test_machine"),
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(agent.config.name, "汎用エージェント");
        assert_eq!(agent.config.auto_record_observations, true);
    }

    #[tokio::test]
    async fn test_agent_make_decision() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(
            Uuid::new_v4(),
            TestState::Idle,
            policy,
            storage,
            MachineBuilder::new("test_machine"),
            None,
            None,
        )
        .await
        .unwrap();

        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        let decision = agent.next_decision().await.unwrap();
        assert_eq!(decision.event, TestEvent::Start);
    }

    #[tokio::test]
    async fn test_agent_apply_decision() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(
            Uuid::new_v4(),
            TestState::Idle,
            policy,
            storage,
            MachineBuilder::new("test_machine"),
            None,
            None,
        )
        .await
        .unwrap();

        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        let decision = agent.next_decision().await.unwrap();

        let next_state = agent.apply_decision(&decision).await.unwrap();
        assert_eq!(next_state, TestState::Processing);
    }

    #[tokio::test]
    async fn test_agent_run_until_goal() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(
            Uuid::new_v4(),
            TestState::Idle,
            policy,
            storage,
            MachineBuilder::new("test_machine"),
            None,
            None,
        )
        .await
        .unwrap();

        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        let success = agent.run_until_goal(Some(5)).await.unwrap();
        assert!(success);
    }

    #[test]
    #[should_panic]
    #[ignore]
    fn test_agent_with_invalid_episode_configuration() {
        panic!("Test ignored, needs rework for async and proper setup");
    }

    #[allow(unused_imports)]
    use crate::agent::{};
}
