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
use rustate::integration::{SharedContext, SharedMachineRef};
use rustate::{Context, EventTrait, Machine, StateTrait};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
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
    S: StateTrait
        + Clone
        + Debug
        + DeserializeOwned
        + Send
        + Sync
        + PartialEq
        + 'static
        + Default
        + Serialize,
    E: EventTrait
        + Clone
        + Debug
        + DeserializeOwned
        + Send
        + Sync
        + 'static
        + rustate::IntoEvent
        + Serialize,
    SM: Storage<S, E>,
    P: Policy<S, E>,
{
    /// エージェントの一意ID
    pub id: Uuid,
    /// エージェントの状態機械（共有参照または所有）
    machine_ref: Option<SharedMachineRef<S, E>>,
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
    shared_context: Option<SharedContext>,
    /// 型パラメータのマーカー
    _phantom: PhantomData<(S, E)>,
}

impl<S, E, SM, P> Agent<S, E, SM, P>
where
    S: StateTrait
        + DeserializeOwned
        + Debug
        + Clone
        + Send
        + Sync
        + PartialEq
        + 'static
        + Default
        + Serialize,
    E: EventTrait
        + DeserializeOwned
        + Debug
        + Clone
        + Send
        + Sync
        + 'static
        + rustate::IntoEvent
        + Serialize,
    SM: Storage<S, E>,
    P: Policy<S, E>,
{
    /// 新しいエージェントを作成します
    pub fn new(machine: Machine<S, E>, policy: P, storage: SM) -> Self {
        Self {
            id: Uuid::new_v4(),
            machine_ref: None,
            machine: Some(machine),
            config: AgentConfig::default(),
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            shared_context: None,
            _phantom: PhantomData,
        }
    }

    /// 共有状態機械参照を使用してエージェントを作成します
    pub fn with_shared_machine(
        machine_ref: SharedMachineRef<S, E>,
        policy: P,
        storage: SM,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            machine_ref: Some(machine_ref),
            machine: None,
            config: AgentConfig::default(),
            policy: Arc::new(policy),
            storage: Arc::new(storage),
            current_episode: None,
            shared_context: None,
            _phantom: PhantomData,
        }
    }

    /// エージェントの設定を変更します
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        // 共有コンテキストの設定
        if config.use_shared_context && self.shared_context.is_none() {
            self.shared_context = Some(SharedContext::new());
        }
        self.config = config;
        self
    }

    /// 共有コンテキストを追加します
    pub fn with_shared_context(mut self, context: SharedContext) -> Self {
        self.shared_context = Some(context);
        // 設定も更新
        self.config.use_shared_context = true;
        self
    }

    /// 現在の状態機械を取得します
    pub fn machine(&self) -> Result<&Machine<S, E>, AgentError> {
        if let Some(ref machine) = self.machine {
            Ok(machine)
        } else if let Some(ref machine_ref) = self.machine_ref {
            machine_ref
                .machine()
                .map_err(|e| AgentError::IntegrationError(e.to_string()))
        } else {
            Err(AgentError::Other(
                "状態機械が設定されていません".to_string(),
            ))
        }
    }

    /// 現在の状態を取得します
    pub fn current_state(&self) -> Result<S, AgentError> {
        if let Some(ref machine) = self.machine {
            Ok(machine.current_state().clone())
        } else if let Some(ref machine_ref) = self.machine_ref {
            machine_ref
                .current_state()
                .map_err(|e| AgentError::IntegrationError(e.to_string()))
                .map(|s| s.clone())
        } else {
            Err(AgentError::Other(
                "状態機械が設定されていません".to_string(),
            ))
        }
    }

    /// 新しいエピソードを開始します
    pub async fn start_episode(
        &mut self,
        name: impl Into<String>,
        goal_state: Option<S>,
    ) -> Result<(), AgentError> {
        // 初期状態を取得
        let initial_state = self.current_state()?;

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
        let previous_state = self.current_state()?;
        let context = if let Some(ref shared_ctx) = self.shared_context {
            // 共有コンテキストから値を取得してContextに変換
            let mut ctx = Context::default();
            // 必要なキーに基づいて共有コンテキストから値を取得
            // この例では簡単のため空のコンテキストを返しています
            ctx
        } else {
            Context::default()
        };

        // イベントを適用
        let next_state = if let Some(ref mut machine) = self.machine {
            match machine.transition(decision.event.clone(), context) {
                Ok(s) => s,
                Err(e) => return Err(AgentError::MachineError(e)),
            }
        } else if let Some(ref machine_ref) = self.machine_ref {
            // 共有参照の場合はsend_eventを使用
            match machine_ref.send_event(decision.event.clone()) {
                Ok(_) => machine_ref
                    .current_state()
                    .map_err(|e| AgentError::IntegrationError(e.to_string()))?
                    .clone(),
                Err(e) => return Err(AgentError::IntegrationError(e.to_string())),
            }
        } else {
            return Err(AgentError::Other(
                "状態機械が設定されていません".to_string(),
            ));
        };

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

            // 共有コンテキストが有効な場合、観測データを保存
            if let Some(ref shared_ctx) = self.shared_context {
                let observation_key = format!("observation_{}", Uuid::new_v4());
                shared_ctx
                    .set(
                        &observation_key,
                        &serde_json::to_string(&observation)
                            .map_err(|e| AgentError::SerializationError(e.to_string()))?,
                    )
                    .map_err(|e| AgentError::IntegrationError(e.to_string()))?;
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
                    episode.add_insight(insight.clone());
                }

                // 共有コンテキストが有効な場合、洞察データを保存
                if let Some(ref shared_ctx) = self.shared_context {
                    let insight_key = format!("insight_{}", Uuid::new_v4());
                    shared_ctx
                        .set(
                            &insight_key,
                            &serde_json::to_string(&insight)
                                .map_err(|e| AgentError::SerializationError(e.to_string()))?,
                        )
                        .map_err(|e| AgentError::IntegrationError(e.to_string()))?;
                }
            }
        }

        Ok(next_state)
    }

    /// 1ステップ実行します（決定して適用）
    pub async fn step(&mut self) -> Result<S, AgentError> {
        let decision = self.next_decision().await?;
        self.apply_decision(&decision).await
    }

    /// 目標状態に達するまで実行します
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> Result<bool, AgentError> {
        // 現在のエピソードがなければエラー
        let episode = match &self.current_episode {
            Some(ep) => ep,
            None => {
                return Err(AgentError::Other(
                    "エピソードが開始されていません".to_string(),
                ))
            }
        };

        // 目標状態を取得
        let goal_state = episode.goal_state.clone();

        // 最大ステップ数
        let max_iterations = max_steps.unwrap_or(100);
        let mut iteration = 0;

        // 現在の状態を取得
        let mut current_state = self.current_state()?;

        // 目標状態に達するまで繰り返す
        while current_state != goal_state && iteration < max_iterations {
            // 次のステップを実行
            current_state = self.step().await?;
            iteration += 1;
        }

        // 目標状態に達したかどうかを返す
        let success = current_state == goal_state;

        // エピソードを完了
        self.complete_episode(success).await?;

        Ok(success)
    }

    /// 洞察を追加します
    pub async fn add_insight(&mut self, insight: Insight) -> Result<(), AgentError> {
        // 洞察を保存
        self.storage.save_insight(&insight).await?;

        // エピソードに洞察を追加
        if let Some(episode) = &mut self.current_episode {
            episode.add_insight(insight.clone());
        }

        // 共有コンテキストが有効な場合、洞察データを保存
        if let Some(ref shared_ctx) = self.shared_context {
            let insight_key = format!("insight_{}", Uuid::new_v4());
            shared_ctx
                .set(
                    &insight_key,
                    &serde_json::to_string(&insight)
                        .map_err(|e| AgentError::SerializationError(e.to_string()))?,
                )
                .map_err(|e| AgentError::IntegrationError(e.to_string()))?;
        }

        Ok(())
    }

    /// フィードバックを追加します
    pub async fn add_feedback(&mut self, feedback: Feedback<E>) -> Result<(), AgentError> {
        // フィードバックを保存
        self.storage.save_feedback(&feedback).await?;

        // エピソードにフィードバックを追加
        if let Some(episode) = &mut self.current_episode {
            episode.add_feedback(feedback.clone());
        }

        // 共有コンテキストが有効な場合、フィードバックデータを保存
        if let Some(ref shared_ctx) = self.shared_context {
            let feedback_key = format!("feedback_{}", Uuid::new_v4());
            shared_ctx
                .set(
                    &feedback_key,
                    &serde_json::to_string(&feedback)
                        .map_err(|e| AgentError::SerializationError(e.to_string()))?,
                )
                .map_err(|e| AgentError::IntegrationError(e.to_string()))?;
        }

        Ok(())
    }

    /// 現在のエピソードを取得します
    pub fn current_episode(&self) -> Option<&Episode<S, E>> {
        self.current_episode.as_ref()
    }

    /// 決定を生成します
    pub async fn make_decision(&self) -> Result<Decision<E>, AgentError> {
        // 現在のエピソードがなければエラー
        let episode = match &self.current_episode {
            Some(ep) => ep,
            None => {
                return Err(AgentError::Other(
                    "エピソードが開始されていません".to_string(),
                ))
            }
        };

        // 現在の状態を取得
        let current_state = self.current_state()?;

        // 決定コンテキストを作成
        let context = DecisionContext {
            current_state: current_state.clone(),
            goal_state: episode.goal_state.clone(),
            observations: episode.observations.clone(),
            feedbacks: episode.feedback.clone().into_iter().collect(),
            insights: episode.insights.clone(),
        };

        // ポリシーを使用して決定を生成
        let decision = self.policy.decide(context).await?;

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{Decision, DecisionMaker};
    use crate::feedback::Feedback;
    use crate::insight::Insight;
    use crate::observation::Observation;
    use crate::policy::Policy;
    use crate::storage::MemoryStorage;
    use rustate::{Machine, MachineBuilder, State as RuState, Transition};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::time::sleep;
    use uuid::Uuid;

    // テスト用の状態
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    // テスト用のイベント
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    // テスト用ポリシー
    struct TestPolicy {
        state_action_map: HashMap<TestState, TestEvent>,
    }

    impl TestPolicy {
        fn new() -> Self {
            let mut map = HashMap::new();
            map.insert(TestState::Idle, TestEvent::Start);
            map.insert(TestState::Processing, TestEvent::Complete);
            map.insert(TestState::Error, TestEvent::Retry);
            map.insert(TestState::Completed, TestEvent::Start); // ループ用
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
        ) -> Result<Decision<TestEvent>, AgentError> {
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

    // テスト用の状態機械を作成
    fn create_test_machine() -> Machine<TestState, TestEvent> {
        // 状態の作成
        let idle = RuState::new("idle", TestState::Idle);
        let processing = RuState::new("processing", TestState::Processing);
        let completed = RuState::new("completed", TestState::Completed);
        let error = RuState::new("error", TestState::Error);

        // 遷移の作成
        let idle_to_processing = Transition::new("idle", "START", "processing");
        let processing_to_completed = Transition::new("processing", "COMPLETE", "completed");
        let processing_to_error = Transition::new("processing", "ABORT", "error");
        let error_to_processing = Transition::new("error", "RETRY", "processing");
        let completed_to_idle = Transition::new("completed", "START", "idle");

        // 状態機械の構築
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

    // 統合機能を使用した共有状態機械のテスト
    #[tokio::test]
    async fn test_agent_with_shared_machine() {
        // 状態機械の作成
        let machine = create_test_machine();

        // 共有参照の作成
        let shared_machine = SharedMachineRef::new(machine);

        // エージェントの作成
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::with_shared_machine(shared_machine.clone(), policy, storage);

        // 目標状態設定
        let goal_state = TestState::Completed;

        // エピソード開始
        agent
            .start_episode("テストエピソード", Some(goal_state))
            .await
            .unwrap();

        // ステップ実行
        let next_state = agent.step().await.unwrap();
        assert_eq!(next_state, TestState::Processing);

        // もう一度ステップ実行
        let final_state = agent.step().await.unwrap();
        assert_eq!(final_state, TestState::Completed);

        // エピソードを完了
        let episode = agent.complete_episode(true).await.unwrap().unwrap();
        assert!(episode.is_completed());
        assert!(episode.is_successful());
    }

    // 共有コンテキストを使用したテスト
    #[tokio::test]
    async fn test_agent_with_shared_context() {
        // 状態機械の作成
        let machine = create_test_machine();

        // 共有コンテキストの作成
        let shared_context = SharedContext::new();

        // エージェントの作成
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent =
            Agent::new(machine, policy, storage).with_shared_context(shared_context.clone());

        // 共有コンテキストに値を設定
        shared_context.set("test_key", "test_value").unwrap();

        // 目標状態設定
        let goal_state = TestState::Completed;

        // エピソード開始
        agent
            .start_episode("テストエピソード", Some(goal_state))
            .await
            .unwrap();

        // 目標状態まで実行
        let success = agent.run_until_goal(Some(5)).await.unwrap();
        assert!(success);

        // 共有コンテキストから値を取得
        let value: Option<String> = shared_context.get("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let agent = Agent::new(machine, policy, storage);

        assert_eq!(agent.config.name, "汎用エージェント");
        assert_eq!(agent.config.auto_record_observations, true);
    }

    #[tokio::test]
    async fn test_agent_make_decision() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        // 決定を取得
        let decision = agent.next_decision().await.unwrap();
        assert_eq!(decision.event, TestEvent::Start);
    }

    #[tokio::test]
    async fn test_agent_apply_decision() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        // 決定を取得
        let decision = agent.next_decision().await.unwrap();

        // 決定を適用
        let next_state = agent.apply_decision(&decision).await.unwrap();
        assert_eq!(next_state, TestState::Processing);
    }

    #[tokio::test]
    async fn test_agent_run_until_goal() {
        let machine = create_test_machine();
        let storage = MemoryStorage::new();
        let policy = TestPolicy::new();
        let mut agent = Agent::new(machine, policy, storage);

        // エピソードを開始
        agent
            .start_episode("テスト", Some(TestState::Completed))
            .await
            .unwrap();

        // 目標まで実行
        let success = agent.run_until_goal(Some(5)).await.unwrap();
        assert!(success);
    }

    #[test]
    fn test_agent_with_invalid_episode_configuration() {
        let agent: Agent<TestState, TestEvent> = Agent::new("Test Agent");

        let decision = Decision::simple(TestEvent::Start, 0.9);

        let result = agent.apply_decision(decision.clone(), None);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No active episode to apply decision to"
        );
    }
}
