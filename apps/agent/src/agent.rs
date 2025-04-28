use crate::error::{self, AgentError, PolicyError, Result as AgentResult};
use crate::feedback::Feedback;
use crate::goal::Goal;
use crate::insight::{Insight, InsightGenerator};
use crate::policy::Policy;
use crate::storage::Storage;
use async_trait::async_trait;
use rustate::{
    Context, Event as RustateEvent, EventTrait as RuEventTrait, IntoEvent as RuIntoEvent, Machine,
    MachineBuilder, SharedMachineRef, State as RustateState, StateTrait as RuStateTrait, StateType,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;

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
pub struct Agent<S, E, SM, P, IG>
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
    E: RuEventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + RuIntoEvent
        + Default
        + Hash
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
    IG: InsightGenerator<S, E> + Send + Sync + 'static,
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
    /// 洞察生成器
    insight_generator: Arc<Mutex<IG>>,
}

impl<S, E, SM, P, IG> Agent<S, E, SM, P, IG>
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
    E: RuEventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + RuIntoEvent
        + Default
        + Hash
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
    IG: InsightGenerator<S, E> + Send + Sync + 'static,
{
    /// 新しいエージェントを作成します
    pub fn new(
        id: impl Into<String>,
        machine_builder: MachineBuilder<(), E, S, S>,
        policy: P,
        storage: SM,
        insight_generator: Arc<Mutex<dyn InsightGenerator<S, E> + Send + Sync>>,
        config: Option<AgentConfig>,
        shared_context: Option<Arc<Mutex<Context>>>,
    ) -> AgentResult<Self> {
        let machine = machine_builder
            .build()
            .map_err(|e| AgentError::InternalError(format!("Machine build failed: {}", e)))?;

        let final_config = config.unwrap_or_default();
        let final_shared_context = if final_config.use_shared_context {
            shared_context.or_else(|| Some(Arc::new(Mutex::new(machine.context.clone()))))
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
            insight_generator,
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
        insight_generator: Arc<Mutex<dyn InsightGenerator<S, E> + Send + Sync>>,
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
            insight_generator,
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
        if let Some(ref _sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "Direct machine access not available when using SharedMachineRef".to_string(),
            ))
        } else {
            self.machine.as_ref().ok_or(AgentError::NotInitialized)
        }
    }

    /// 現在の状態を取得します (May fail if using SharedMachineRef)
    pub fn current_state(&self) -> AgentResult<S> {
        if let Some(ref sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "current_state via SharedMachineRef not implemented".to_string(),
            ))
        } else {
            self.machine().and_then(|m| Ok(m.state().clone()))
        }
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
        // Ensure initial state is valid for the machine
        // let current_machine_state = self.current_state()?;
        // It might be better to reset the machine to the initial_state if provided
        // self.reset_machine_state(initial_state.clone()).await?;

        let episode = Episode::new(name.into(), initial_state.clone(), goal_obj);
        self.current_episode = Some(episode.clone());

        self.storage.save_episode(&episode).await?;
        Ok(())
    }

    /// Complete the current episode.
    pub async fn complete_episode(
        &mut self,
        is_successful: bool,
    ) -> AgentResult<Option<Episode<S, E>>> {
        if let Some(mut episode) = self.current_episode.take() {
            episode.complete(is_successful);
            self.storage.save_episode(&episode).await?;

            if self.config.auto_generate_insights {
                // Temporarily comment out insight generation due to policy.rs changes
                /*
                match self.generate_insights(&episode).await {
                    Ok(insights) => {
                        for insight in insights {
                            self.storage.save_insight(&insight).await?; // Use corrected signature
                        }
                    }
                    Err(_e) => {} // log::error!("Failed to generate insights: {}", e),
                }
                */
            }

            Ok(Some(episode))
        } else {
            Ok(None) // No active episode to complete
        }
    }

    /// Get the next decision from the policy based on the current state.
    pub async fn next_decision(&self) -> AgentResult<Decision<E>> {
        if let Some(episode) = &self.current_episode {
            let current_state = self.current_state()?;
            let goal_state = episode.goal.target_state.clone();
            let episode_id_str = episode.id.to_string();
            let observations = self.storage.get_observation(&episode_id_str).await?;
            let feedbacks = self.storage.get_feedback(&episode_id_str).await?;
            let insights = self.storage.get_insight(&episode_id_str).await?;

            // TODO: get_observation/get_insight return single items, but DecisionContext expects Vec.
            // Need to adjust storage trait/impls or how context is built.
            // For now, wrapping in vec! as placeholder.
            let _decision_context = DecisionContext::new(
                current_state.clone(), // Clone current state for context
                goal_state.clone(),    // Pass goal_state directly (already cloned)
                vec![observations],    // Placeholder
                vec![feedbacks],       // Placeholder
                vec![insights],        // Placeholder
            );
            self.policy.decide(current_state, goal_state).await
        } else {
            Err(AgentError::NoActiveEpisode)
        }
    }

    /// Executes a single step in the agent's decision-making process.
    pub async fn step(&mut self) -> AgentResult<S> {
        // Get current state and decision first (immutable borrows)
        let _current_state = self.current_state()?;
        let decision = self.make_decision().await?;

        // Clone episode data needed *before* applying decision
        let episode_id;
        {
            let episode = self
                .current_episode
                .as_mut()
                .ok_or(AgentError::NoActiveEpisode)?;
            episode.add_decision(decision.clone());
            episode_id = episode.id; // Clone ID
                                     // Mutable borrow ends here
        }

        // Apply decision (immutable borrow)
        let next_state = self.apply_decision(&decision).await?;

        // Check if goal reached (immutable borrow)
        let goal_reached = self.is_goal_reached(&next_state)?;

        if goal_reached {
            // complete_episode takes &mut self and handles saving internally
            self.complete_episode(true).await?;
        } else {
            // If not completed, get the episode again (mutably) and save
            // This re-borrow is fine as other borrows are finished
            let episode_to_save = self.current_episode.as_mut().ok_or_else(|| {
                AgentError::InternalError(format!(
                    "Episode {} disappeared unexpectedly after step but before save",
                    episode_id
                ))
            })?;
            // storage.save_episode takes &Episode
            self.storage.save_episode(episode_to_save).await?;
        }

        Ok(next_state)
    }

    /// Runs the agent until the goal state is reached or max_steps are exceeded.
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> AgentResult<bool> {
        let mut steps = 0;
        loop {
            if let Some(max) = max_steps {
                if steps >= max {
                    self.complete_episode(false).await?; // Mark as unsuccessful
                    return Ok(false); // Max steps reached
                }
            }

            // Get current state first
            let current_state = self.current_state()?;

            // Check if the current state is already the goal before taking a step
            if self.is_goal_reached(&current_state)? {
                // Ensure the episode is marked successful even if it starts at the goal
                self.complete_episode(true).await?;
                return Ok(true);
            }

            // If not already at goal, take a step
            let next_state = self.step().await?;
            steps += 1;

            // Check if the goal was reached after the step
            if self.is_goal_reached(&next_state)? {
                // step() already called complete_episode
                return Ok(true);
            }

            // Additional check: If the machine entered a final state not necessarily the goal
            if let Ok(machine) = self.machine() {
                let is_final = machine.current_states.iter().any(|s_id| {
                    machine
                        .states
                        .get(s_id)
                        .is_some_and(|s| s.state_type == StateType::Final)
                });
                if is_final {
                    // Reached a final state, but not the goal
                    self.complete_episode(false).await?;
                    return Ok(false);
                }
            }
            // Need similar check for SharedMachineRef if possible
        }
    }

    /// Process an external event through the state machine.
    pub async fn process_event(&self, event: E) -> AgentResult<S> {
        let result = if let Some(ref sm_ref) = self.machine_ref {
            return Err(AgentError::NotSupported(
                "process_event via SharedMachineRef not implemented".to_string(),
            ));
        } else if let Some(machine) = self.machine.as_ref() {
            machine
                .send(event)
                .await
                .map_err(|e| AgentError::InternalError(format!("State transition failed: {}", e)))?
        } else {
            return Err(AgentError::NotInitialized);
        };

        if result {
            self.current_state()
        } else {
            self.current_state()
        }
    }

    /// Add an insight to the agent's knowledge base.
    pub async fn add_insight(&mut self, insight: Insight) -> AgentResult<()> {
        self.storage.save_insight(&insight).await?;
        Ok(())
    }

    /// Add feedback to the agent's experience.
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> AgentResult<()> {
        self.process_feedback(&feedback).await?;
        Ok(())
    }

    /// Provides access to the current episode, if active.
    pub fn current_episode(&self) -> Option<&Episode<S, E>> {
        self.current_episode.as_ref()
    }

    /// Make a decision based on the current context (internal helper potentially).
    async fn make_decision(&self) -> AgentResult<Decision<E>> {
        let current_state = self.current_state()?;
        let goal_state = self
            .current_episode
            .as_ref()
            .ok_or(AgentError::NoActiveEpisode)?
            .goal
            .target_state
            .clone();

        // Create DecisionContext (ensure all args are provided)
        // Note: DecisionContext::new expects current_state, goal_state, observations, feedbacks, insights
        let _decision_context: DecisionContext<S, E> = DecisionContext::new(
            current_state.clone(), // Pass current state
            goal_state.clone(),    // Pass goal state directly
            Vec::new(),            // Placeholder for observations
            Vec::new(),            // Placeholder for feedbacks
            Vec::new(),            // Placeholder for insights
        );

        // Use the policy to make a decision based on the context
        // Revert to original call signature based on current Policy trait definition
        self.policy.decide(current_state, goal_state).await
    }

    /// Get the policy instance.
    pub fn policy(&self) -> Arc<P> {
        self.policy.clone()
    }

    /// Get the storage manager instance.
    pub fn storage(&self) -> Arc<SM> {
        self.storage.clone()
    }

    /// Internal method to process feedback.
    pub async fn process_feedback(&self, feedback: &Feedback<E>) -> AgentResult<()> {
        self.insight_generator
            .lock()
            .await
            .apply_feedback(feedback)
            .await
            .map_err(|e| AgentError::InternalError(format!("Feedback application failed: {}", e)))
    }

    /// Helper to generate insights (example implementation).
    pub async fn generate_insights(&self, episode: &Episode<S, E>) -> AgentResult<Vec<Insight>> {
        self.insight_generator
            .lock()
            .await
            .generate_insights(episode)
            .await
            .map_err(|e| AgentError::InternalError(format!("Insight generation failed: {}", e)))
            .map(|strings| strings.into_iter().map(Insight::new).collect())
    }

    /// Applies a decision by sending the event to the state machine.
    async fn apply_decision(&self, decision: &Decision<E>) -> AgentResult<S> {
        let event_to_send = decision.event.clone();
        self.process_event(event_to_send).await
    }

    /// Checks if the current state matches the goal state of the active episode.
    fn is_goal_reached(&self, current_state: &S) -> AgentResult<bool> {
        if let Some(ep) = &self.current_episode {
            Ok(current_state == &ep.goal.target_state)
        } else {
            Ok(current_state.state_type() == StateType::Final)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Bring parent scope items into tests
    use crate::agent::AgentId;
    use crate::decision::Decision;
    use crate::episode::Episode;
    use crate::error::{self, AgentError, PolicyError, StorageError, Result as AgentResult};
    use crate::feedback::Feedback;
    use crate::goal::Goal;
    use crate::insight::{Insight, InsightGenerator};
    use crate::observation::Observation;
    use crate::policy::Policy;
    use crate::storage::{MemoryStorage, Storage};
    use async_trait::async_trait;
    use rustate::{
        Context, Event as RustateEvent, EventTrait, IntoEvent, Machine, MachineBuilder,
        State as RustateState, StateMachine as RuStateMachine, StateTrait, StateType,
    };
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fmt::{self, Debug, Display, Formatter};
    use std::hash::Hash;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    // Define TestState locally for agent tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    enum TestState {
        Idle,
        Running,
        Stopped,
    }

    // Required impl for StateTrait
    impl Display for TestState {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    // Required impl for MachineBuilder
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

    // Required impl for Agent/MachineBuilder
    impl Default for TestState {
        fn default() -> Self {
            TestState::Idle
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &Self {
            self // ID is the state itself for enums usually
        }

        fn state_type(&self) -> StateType {
            match self {
                TestState::Idle => StateType::Initial,
                TestState::Running => StateType::Intermediate,
                TestState::Stopped => StateType::Final,
            }
        }
        // box_clone is NOT part of StateTrait in rustate 0.3
        // Add other methods if required by the specific version of StateTrait
    }

    // Define TestEvent locally for agent tests
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    enum TestEvent {
        Start,
        Stop,
        NoOp,
    }

    // Required impl for Agent/MachineBuilder
    impl Default for TestEvent {
        fn default() -> Self {
            TestEvent::NoOp // Or another sensible default
        }
    }

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "Start",
                TestEvent::Stop => "Stop",
                TestEvent::NoOp => "NoOp",
            }
        }

        fn payload(&self) -> Option<&serde_json::Value> {
            None // No payload for these simple events
        }

        fn name(&self) -> &str {
            self.event_type()
        }
        // id and box_clone are NOT part of EventTrait
    }

    // IntoEvent does not take generics in rustate 0.3
    impl IntoEvent for TestEvent {
        fn into_event(self) -> RustateEvent {
            RustateEvent::new(self.name()) // Use the correct Event struct
        }
    }

    // Mock Policy
    struct MockPolicy;

    #[async_trait]
    impl Policy<TestState, TestEvent> for MockPolicy {
        async fn decide(
            &self,
            state: RustateState<TestState>,
            _goal_state: RustateState<TestState>,
        ) -> crate::error::Result<Decision<TestEvent>> {
            match state.get_state() {
                TestState::Idle => Ok(Decision::new(Some(TestEvent::Start))),
                TestState::Running => Ok(Decision::new(Some(TestEvent::Stop))),
                TestState::Stopped => Ok(Decision::new(Some(TestEvent::NoOp))),
            }
        }
    }

    // Mock Insight Generator
    struct MockInsightGenerator;

    // Assuming InsightGenerator is a trait defined in crate::insight
    impl InsightGenerator<TestState, TestEvent> for MockInsightGenerator {
        fn generate_insights(
            &self,
            _episode: &Episode<TestState, TestEvent>,
        ) -> crate::error::Result<Vec<String>> {
            // Use crate::error::Result
            Ok(vec!["mock_insight".to_string()])
        }

        fn apply_feedback(
            &mut self,
            _feedback: &Feedback<TestState, TestEvent>,
        ) -> crate::error::Result<()> {
            // Use crate::error::Result
            Ok(())
        }
    }

    // Helper function using local types
    fn create_test_machine_builder() -> MachineBuilder<(), TestEvent, TestState, TestState> {
        let idle = RustateState::new(TestState::Idle);
        let running = RustateState::new(TestState::Running);
        let stopped = RustateState::new(TestState::Stopped);

        MachineBuilder::new("test_machine", idle.clone())
            .state(running.clone())
            .state(stopped.clone())
            .transition(
                idle.id(),
                TestEvent::Start, // Use enum directly
                running.id(),
                None,
                None,
            )
            .transition(running.id(), TestEvent::Stop, stopped.id(), None, None)
            .transition(stopped.id(), TestEvent::Stop, stopped.id(), None, None)
    }

    // Helper to create agent with mocks
    type MockStateMachine = StateMachine<(), TestEvent, TestState, TestState>;
    type MockPolicyImpl = MockPolicy;

    // Agent P generic should be the Policy impl, not the Arc<Mutex<...>>
    fn create_test_agent(
    ) -> Agent<TestState, TestEvent, MockStateMachine, MockPolicyImpl, MockInsightGenerator> {
        let agent_id = AgentId::new();
        // Policy is wrapped in Arc internally by Agent::new
        let policy = MockPolicy;
        let insight_generator = Arc::new(Mutex::new(MockInsightGenerator));
        let initial_state = RustateState::new(TestState::Idle);
        let goal = Goal {
            target_state: TestState::Stopped,
        };
        let machine_builder = create_test_machine_builder();
        let state_machine = machine_builder
            .build()
            .expect("Failed to build test state machine");

        Agent::new(
            agent_id,
            policy, // Pass the unwrapped policy
            insight_generator,
            initial_state,
            state_machine,
            Some(goal),
            None,
        )
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = create_test_agent();
        assert_eq!(agent.id().to_string().len(), 36);
        assert_eq!(agent.current_state.get_state(), &TestState::Idle);
        assert!(agent.goal.is_some());
        assert_eq!(
            agent.goal.as_ref().unwrap().target_state,
            TestState::Stopped
        );
    }

    #[tokio::test]
    async fn test_agent_run_step() {
        let mut agent = create_test_agent();

        // Step 1: Idle -> Running
        let result1 = agent.run_step().await;
        assert!(result1.is_ok(), "Step 1 failed: {:?}", result1.err());
        let observation1 = result1
            .unwrap()
            .expect("Step 1 should produce an observation");
        assert_eq!(observation1.state.get_state(), &TestState::Idle);
        assert_eq!(
            observation1.event.expect("Event missing").get_event(),
            &TestEvent::Start
        );
        assert_eq!(agent.current_state.get_state(), &TestState::Running);

        // Step 2: Running -> Stopped
        let result2 = agent.run_step().await;
        assert!(result2.is_ok(), "Step 2 failed: {:?}", result2.err());
        let observation2 = result2
            .unwrap()
            .expect("Step 2 should produce an observation");
        assert_eq!(observation2.state.get_state(), &TestState::Running);
        assert_eq!(
            observation2.event.expect("Event missing").get_event(),
            &TestEvent::Stop
        );
        assert_eq!(agent.current_state.get_state(), &TestState::Stopped);

        // Step 3: Stopped -> NoOp (Agent should stop as goal reached)
        let result3 = agent.run_step().await;
        assert!(result3.is_ok(), "Step 3 failed: {:?}", result3.err());
        assert!(result3.unwrap().is_none(), "Agent should stop at goal");
        assert_eq!(agent.current_state.get_state(), &TestState::Stopped);
    }

    #[tokio::test]
    async fn test_agent_process_feedback() {
        let mut agent = create_test_agent();
        let observation = Observation::new(
            RustateState::new(TestState::Idle),
            Some(RustateEvent::new(TestEvent::Start.name())), // Use correct Event construction
        );
        let episode = Episode::new(agent.id(), vec![observation]);
        let feedback = Feedback::new(episode, 1.0, HashMap::new());

        let result = agent.process_feedback(feedback).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_get_insights() {
        let agent = create_test_agent();
        let observation = Observation::new(
            RustateState::new(TestState::Idle),
            Some(RustateEvent::new(TestEvent::Start.name())), // Use correct Event construction
        );
        let episode = Episode::new(agent.id(), vec![observation]);

        let insights = agent.get_insights(&episode);
        assert!(insights.is_ok());
        assert_eq!(insights.unwrap(), vec!["mock_insight".to_string()]);
    }
}
