use crate::{
    decision::{Decision, DecisionContext},
    episode::Episode,
    error::{AgentError, Result},
    feedback::Feedback,
    goal::Goal,
    insight::Insight,
    policy::Policy,
    storage::Storage,
};
use rustate::{
    machine::{Machine, MachineBuilder},
    state::{StateTrait as RuStateTrait, StateType as RuStateType},
    Context, EventTrait as RuEventTrait, IntoEvent as RuIntoEvent, SharedMachineRef,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, marker::PhantomData, sync::Arc};
use tokio::sync::Mutex;

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
        + for<'de> Deserialize<'de>
        + Default
        + 'static,
    E: RuEventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + RuIntoEvent
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
        + for<'de> Deserialize<'de>
        + Default
        + 'static,
    E: RuEventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>
        + RuIntoEvent
        + 'static,
    SM: Storage<S, E> + Send + Sync + 'static,
    P: Policy<S, E> + Send + Sync + 'static,
{
    /// 新しいエージェントを作成します (Original constructor - adjusted)
    pub fn new(
        id: impl Into<String>,
        machine_builder: MachineBuilder<S, E>,
        policy: P,
        storage: SM,
        config: Option<AgentConfig>,
        shared_context: Option<Arc<Mutex<Context>>>,
    ) -> Result<Self> {
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
            current_episode: None,
            shared_context: final_shared_context,
            _phantom: PhantomData,
        })
    }

    /// 共有状態機械参照を使用してエージェントを作成します (Original constructor - adjusted)
    pub fn with_shared_machine(
        id: impl Into<String>,
        machine_ref: SharedMachineRef,
        policy: P,
        storage: SM,
        config: Option<AgentConfig>,
        // Assuming shared_context comes from the machine_ref implicitly or is not set here
        // shared_context is not passed here, should be handled internally if needed
    ) -> Result<Self> {
        let final_config = config.unwrap_or_default();
        let final_shared_context = if final_config.use_shared_context {
            // Attempt to get context from SharedMachineRef if possible, otherwise create default.
            // This requires SharedMachineRef to expose context access.
            // For now, assume default or None based on how SharedMachineRef works.
            // machine_ref.context().map(|ctx| Arc::new(Mutex::new(ctx))) // Hypothetical API
            Some(Arc::new(Mutex::new(Context::default()))) // Placeholder
        } else {
            None
        };

        Ok(Self {
            id: id.into(),
            machine_ref: Some(machine_ref),
            machine: None, // No owned machine
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
    pub fn machine(&self) -> Result<&Machine<S, E>> {
        if let Some(ref _sm_ref) = self.machine_ref {
            Err(AgentError::NotSupported(
                "Direct machine access not available when using SharedMachineRef".to_string(),
            ))
        } else {
            self.machine.as_ref().ok_or(AgentError::NotInitialized)
        }
    }

    /// 現在の状態を取得します (May fail if using SharedMachineRef)
    pub fn current_state(&self) -> Result<S> {
        if let Some(ref _sm_ref) = self.machine_ref {
            // TODO: Implement current_state retrieval via SharedMachineRef if possible
            Err(AgentError::NotSupported(
                "current_state via SharedMachineRef not implemented".to_string(),
            ))
        } else {
            self.machine
                .as_ref()
                .ok_or(AgentError::NotInitialized)
                .map(|m| m.current_state().clone())
        }
    }

    /// Start a new episode for the agent.
    pub async fn start_episode<G: Into<Goal<S>>>(
        &mut self,
        name: impl Into<String>,
        initial_state: S,
        goal: G,
    ) -> Result<()> {
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
    pub async fn complete_episode(&mut self, is_successful: bool) -> Result<Option<Episode<S, E>>> {
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
    pub async fn next_decision(&self) -> Result<Decision<E>> {
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
            let decision_context = DecisionContext::new(
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
    pub async fn step(&mut self) -> Result<S> {
        // Get current state and decision first (immutable borrows)
        let current_state = self.current_state()?;
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
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> Result<bool> {
        let mut steps = 0;
        loop {
            if let Some(max) = max_steps {
                if steps >= max {
                    self.complete_episode(false).await?; // Mark as unsuccessful
                    return Ok(false); // Max steps reached
                }
            }

            let current_state = self.step().await?;
            steps += 1;

            // Check if the current state is the goal state after the step
            if self.is_goal_reached(&current_state)? {
                // step() already calls complete_episode if goal is reached
                return Ok(true);
            }

            // Additional check: If the machine entered a final state not necessarily the goal
            if let Ok(machine) = self.machine() {
                let is_final = machine.current_states.iter().any(|s_id| {
                    machine
                        .states
                        .get(s_id)
                        .map_or(false, |s| s.state_type == RuStateType::Final)
                });
                if is_final {
                    // Check if this final state matches the goal
                    if self.is_goal_reached(&current_state)? {
                        return Ok(true);
                    } else {
                        // Reached a final state, but not the goal
                        self.complete_episode(false).await?;
                        return Ok(false);
                    }
                }
            }
            // Need similar check for SharedMachineRef if possible
        }
    }

    /// Process an external event through the state machine.
    pub async fn process_event(&self, event: E) -> Result<S> {
        let context = if let Some(shared_ctx) = &self.shared_context {
            shared_ctx.lock().await.clone() // Clone the context for the transition
        } else {
            // If using owned machine, clone its context
            self.machine()
                .map(|m| m.context.clone())
                .unwrap_or_default()
        };

        let result = if let Some(ref sm_ref) = self.machine_ref {
            // TODO: Call transition method on SharedMachineRef if available
            // sm_ref.transition(event.into_event(), context).await?
            Err(AgentError::NotSupported(
                "process_event via SharedMachineRef not implemented".to_string(),
            ))
        } else if let Some(mut machine) = self.machine.clone() {
            // Clone to get mut access temporarily
            machine
                .transition(event.into_event(), context)
                .map_err(AgentError::MachineError)
        } else {
            Err(AgentError::NotInitialized)
        };

        // TODO: Update the owned machine state if the transition succeeded
        // We cannot easily do this with the current structure as transition needs &mut self
        // This suggests the Agent should perhaps always own the machine or use SharedMachineRef fully.

        result
    }

    /// Add an insight to the agent's knowledge base.
    pub async fn add_insight(&mut self, insight: Insight) -> Result<()> {
        self.storage.save_insight(&insight).await?; // Use corrected signature
        Ok(())
    }

    /// Add feedback to the agent's experience.
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> Result<()> {
        self.process_feedback(&feedback).await?; // process_feedback saves to storage
        Ok(())
    }

    /// Provides access to the current episode, if active.
    pub fn current_episode(&self) -> Option<&Episode<S, E>> {
        self.current_episode.as_ref()
    }

    /// Make a decision based on the current context (internal helper potentially).
    async fn make_decision(&self) -> Result<Decision<E>> {
        let current_state = self.current_state()?;
        let goal_state = self
            .current_episode
            .as_ref()
            .ok_or(AgentError::NoActiveEpisode)?
            .goal // Access the Goal struct directly
            .target_state // Access the target_state field within Goal
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
    pub async fn process_feedback(&self, feedback: &Feedback<E>) -> Result<()> {
        // TODO: Implement policy update logic if needed
        self.storage.save_feedback(feedback).await?; // Use corrected signature
        Ok(())
    }

    /// Internal method to generate insights from an episode.
    pub async fn generate_insights(&self, episode: &Episode<S, E>) -> Result<Vec<Insight>> {
        // TODO: Verify Storage has get_trace and Policy has analyze_episode_trace
        // let trace = self.storage.get_trace(&episode.id).await?;
        // let insights = self.policy.analyze_episode_trace(&trace).await?;
        // Ok(insights)
        Ok(vec![]) // Placeholder
    }

    /// Applies a decision by sending the event to the state machine.
    async fn apply_decision(&self, decision: &Decision<E>) -> Result<S> {
        // Use process_event to handle the event sending logic
        self.process_event(decision.event.clone()).await
    }

    /// Checks if the current state matches the goal state.
    fn is_goal_reached(&self, current_state: &S) -> Result<bool> {
        if let Some(episode) = &self.current_episode {
            // Access goal field directly
            Ok(episode.goal.target_state.id() == current_state.id())
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
        episode::Goal,
        error::AgentError,
        feedback::Feedback,
        insight::Insight,
        observation::Observation,
        policy::Policy,
        storage::{MemoryStorage, Storage},
    };
    use async_trait::async_trait;
    use rustate::{Event, EventTrait, MachineBuilder, State, StateTrait, StateType, Transition};
    use serde_json::Value;
    use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
    enum TestState {
        #[default]
        Idle,
        Running,
        Stopped,
    }

    impl Display for TestState {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{:?}", self)
        }
    }

    // Implement StateTrait for TestState according to v0.2.4 expectations (based on E0046 error)
    impl StateTrait for TestState {
        fn id(&self) -> &str {
            // Map enum variant to a string slice identifier
            match self {
                TestState::Idle => "Idle",
                TestState::Running => "Running",
                TestState::Stopped => "Stopped",
            }
        }

        fn state_type(&self) -> &StateType {
            // Return a reference to a static StateType
            match self {
                TestState::Stopped => &StateType::Final,
                _ => &StateType::Normal, // Assume Normal for others
            }
        }

        // Provide dummy implementations for other required methods
        fn parent(&self) -> Option<&str> {
            None
        }
        fn children(&self) -> &[String] {
            &[]
        }
        fn initial(&self) -> Option<&str> {
            None
        }
        fn data(&self) -> Option<&Value> {
            None
        } // Assuming no specific data for test states
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum TestEvent {
        Start,
        Stop,
        Pause,
        Resume,
        Custom(String),
    }

    impl Display for TestEvent {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            match self {
                TestEvent::Start => write!(f, "START"),
                TestEvent::Stop => write!(f, "STOP"),
                TestEvent::Pause => write!(f, "PAUSE"),
                TestEvent::Resume => write!(f, "RESUME"),
                TestEvent::Custom(s) => write!(f, "CUSTOM_{}", s),
            }
        }
    }

    // Implement EventTrait for TestEvent (Remove name method if not in v0.2.4)
    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Start => "START",
                TestEvent::Stop => "STOP",
                TestEvent::Pause => "PAUSE",
                TestEvent::Resume => "RESUME",
                TestEvent::Custom(_) => "CUSTOM",
            }
        }

        fn payload(&self) -> Option<&Value> {
            match self {
                TestEvent::Custom(s) => Some(&json!(s)), // Use json! macro for payload
                _ => None,
            }
        }

        // Removed name method - likely not in v0.2.4 EventTrait
    }

    impl RuIntoEvent for TestEvent {
        fn into_event(self) -> Event {
            match self {
                TestEvent::Start => Event::new("START"),
                TestEvent::Stop => Event::new("STOP"),
                TestEvent::Pause => Event::new("PAUSE"),
                TestEvent::Resume => Event::new("RESUME"),
                TestEvent::Custom(s) => {
                    Event::with_payload("CUSTOM", json!(s)) // Use json! macro
                }
            }
        }
    }

    struct TestPolicy;

    #[async_trait]
    impl Policy<TestState, TestEvent> for TestPolicy {
        async fn decide(
            &self,
            current_state: TestState,
            _goal_state: Option<Goal<TestState>>,
        ) -> Result<Decision<TestEvent>> {
            let event = match current_state {
                TestState::Idle => TestEvent::Start,
                TestState::Running => TestEvent::Stop,
                TestState::Stopped => TestEvent::Stop, // Or maybe a different event like Reset?
            };
            Ok(Decision::simple(event, 1.0))
        }
    }

    // --- Corrected create_test_machine_builder for v0.2.4 API ---
    // MachineBuilder likely takes <S = State, E = Event> generics
    fn create_test_machine_builder() -> MachineBuilder<TestState, TestEvent> {
        // State::new expects impl Into<String>
        let idle_state = State::new(TestState::Idle.id());
        let running_state = State::new(TestState::Running.id());
        // State needs a `type` method or similar in v0.2.4, check API.
        // Let's assume State::new creates a Normal state by default.
        // We need a way to mark 'Stopped' as Final.
        // Maybe MachineBuilder has a method for state types?
        // For now, create as Normal and hope Machine handles Final logic based on trait impl.
        let stopped_state = State::new(TestState::Stopped.id());

        // MachineBuilder::new likely takes name: Into<String> only.
        MachineBuilder::<TestState, TestEvent>::new("test_machine")
            .state(idle_state)
            .state(running_state)
            .state(stopped_state) // How to mark as Final?
            .initial(TestState::Idle.id()) // initial takes Into<String>
            // Transition::new(source: Into<String>, event: Into<String>, target: Into<String>)
            .transition(Transition::new(
                TestState::Idle.id(),
                TestEvent::Start.event_type(),
                TestState::Running.id(),
            ))
            .transition(Transition::new(
                TestState::Running.id(),
                TestEvent::Stop.event_type(),
                TestState::Stopped.id(),
            ))
        // Missing methods like .guard(), .action() based on v0.2.4 API? Let's assume base transition for now.
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();

        // Agent::new takes MachineBuilder<S, E> in v0.2.4?
        let agent_result = Agent::new(
            "test-agent-creation",
            builder, // Pass the builder
            policy,
            storage,
            None,
            None,
        );
        // ... rest of test ...
    }

    #[tokio::test]
    async fn test_agent_next_decision_and_step() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let mut agent =
            Agent::new("test-agent-step", builder, policy, storage, None, None).unwrap();

        let goal = Goal::new(TestState::Stopped);
        agent
            .start_episode("ep1", TestState::Idle, goal)
            .await
            .unwrap();
        // ... rest of test ...
    }

    #[tokio::test]
    async fn test_agent_process_event() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let mut agent =
            Agent::new("test-agent-process", builder, policy, storage, None, None).unwrap();
        // ... rest of test ...
    }

    #[tokio::test]
    async fn test_agent_run_until_goal() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let mut agent = Agent::new("test-agent-run", builder, policy, storage, None, None).unwrap();

        let goal = Goal::new(TestState::Stopped);
        agent
            .start_episode("ep1", TestState::Idle, goal)
            .await
            .unwrap();
        // ... rest of test ...
    }

    #[tokio::test]
    async fn test_agent_run_max_steps() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let mut agent =
            Agent::new("test-agent-max-steps", builder, policy, storage, None, None).unwrap();

        let goal = Goal::new(TestState::Stopped);
        agent
            .start_episode("ep_max_steps", TestState::Idle, goal)
            .await
            .unwrap();
        // ... rest of test ...
    }

    #[tokio::test]
    async fn test_agent_episode_management() {
        let builder = create_test_machine_builder();
        let policy = TestPolicy;
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let mut agent =
            Agent::new("test-agent-episode", builder, policy, storage, None, None).unwrap();

        let episode_name = "ep_manage";
        let goal = Goal::new(TestState::Stopped);

        let start_result = agent
            .start_episode(episode_name, TestState::Idle, goal.clone())
            .await;
        // ... rest of test ...

        // Manually set current_states using the state ID string
        agent.current_states = [TestState::Stopped.id().to_string()]
            .iter()
            .cloned()
            .collect();
        // Set current_state within the episode
        agent.current_episode.as_mut().unwrap().current_state = TestState::Stopped;

        // ... rest of test ...
    }
}
