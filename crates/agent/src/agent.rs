use crate::{
    decision::{Decision, DecisionMaker},
    episode::Episode,
    error::{AgentError, Result},
    insight::Insight,
    observation::Observation,
    policy::Policy,
    storage::Storage,
};
use async_trait::async_trait;
use rustate::{Context, Event, Machine, State};
use serde::{de::DeserializeOwned, Serialize};
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
            name: "Rustate Agent".to_string(),
            description: "状態機械駆動のAIエージェント".to_string(),
            max_observations: Some(1000),
            auto_record_observations: true,
            auto_generate_insights: false,
        }
    }
}

/// エージェントの構造体
pub struct Agent<S, E, P, T>
where
    S: State,
    E: Event,
    P: Policy<S, E>,
    T: Storage<S, E>,
{
    /// エージェントの状態機械
    pub machine: Machine<S, E>,
    /// エージェントの設定
    pub config: AgentConfig,
    /// エージェントの決定ポリシー
    policy: Arc<P>,
    /// エージェントのストレージ
    storage: Arc<T>,
    /// 現在のエピソード（ある場合）
    current_episode: Option<Episode<S, E>>,
    /// 型パラメータのマーカー
    _phantom: PhantomData<(S, E)>,
}

impl<S, E, P, T> Agent<S, E, P, T>
where
    S: State + DeserializeOwned + Debug + Clone + Send + Sync + 'static,
    E: Event + DeserializeOwned + Debug + Clone + Send + Sync + 'static,
    P: Policy<S, E> + 'static,
    T: Storage<S, E> + 'static,
{
    /// 新しいエージェントを作成します
    pub fn new(machine: Machine<S, E>, policy: P, storage: T) -> Self {
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
    ) -> Result<()> {
        let initial_state = self.machine.current_state().clone();
        let episode = Episode::new(name.into(), initial_state, goal_state);
        self.current_episode = Some(episode);
        Ok(())
    }

    /// 現在のエピソードを完了します
    pub async fn complete_episode(&mut self, is_successful: bool) -> Result<Option<Episode<S, E>>> {
        if let Some(mut episode) = self.current_episode.take() {
            episode.complete(is_successful);
            self.storage.save_episode(&episode).await?;
            return Ok(Some(episode));
        }
        Ok(None)
    }

    /// エージェントの次の決定を取得します
    pub async fn next_decision(&self) -> Result<Decision<E>> {
        let current_state = self.machine.current_state();
        
        // 目標状態を取得
        let goal_state = self.current_episode
            .as_ref()
            .and_then(|ep| ep.goal_state.as_ref());
        
        // 最近の観測データを取得
        let observations = self.storage
            .find_observations(None, self.config.max_observations)
            .await?;
        
        // 洞察を取得
        let insights = self.storage
            .find_insights(None, None)
            .await?;
        
        // ポリシーを使用して決定を作成
        let decision = self.policy
            .decide(current_state, goal_state, &observations, &insights)
            .await?;
        
        // 決定を保存
        self.storage.save_decision(&decision).await?;
        
        // エピソードに決定を追加
        if let Some(episode) = &self.current_episode {
            let mut episode = episode.clone();
            episode.add_decision(decision.clone());
            // 注: ここでは可変参照の問題を避けるために非効率的なクローンを使用していますが、
            // 実際の実装ではより良い方法を検討すべきです
            let this = unsafe { &mut *(self as *const Self as *mut Self) };
            this.current_episode = Some(episode);
        }
        
        Ok(decision)
    }

    /// 決定に基づいてイベントを適用します
    pub async fn apply_decision(&mut self, decision: &Decision<E>) -> Result<S> {
        let previous_state = self.machine.current_state().clone();
        
        // イベントを適用
        match self.machine.transition(&decision.event, Context::default()) {
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
                            format!("{:?}から{:?}への遷移が観測されました", previous_state, next_state),
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
    pub async fn step(&mut self) -> Result<S> {
        let decision = self.next_decision().await?;
        self.apply_decision(&decision).await
    }

    /// エージェントを目標状態に到達するまで実行します
    pub async fn run_until_goal(&mut self, max_steps: Option<usize>) -> Result<bool> {
        let goal_state = match &self.current_episode {
            Some(episode) => match &episode.goal_state {
                Some(goal) => goal.clone(),
                None => return Err(AgentError::Other("目標状態が設定されていません".to_string())),
            },
            None => return Err(AgentError::Other("エピソードが開始されていません".to_string())),
        };

        let mut steps = 0;
        let max_steps = max_steps.unwrap_or(100); // デフォルトの最大ステップ数

        while steps < max_steps {
            steps += 1;
            
            // 現在の状態をチェック
            if self.machine.current_state() == &goal_state {
                self.complete_episode(true).await?;
                return Ok(true);
            }

            // 次のステップを実行
            self.step().await?;
        }

        // 最大ステップ数に達しても目標に到達しなかった
        self.complete_episode(false).await?;
        Ok(false)
    }

    /// 新しい洞察を追加します
    pub async fn add_insight(&mut self, insight: Insight) -> Result<()> {
        self.storage.save_insight(&insight).await?;
        
        // エピソードに洞察を追加
        if let Some(episode) = &mut self.current_episode {
            episode.add_insight(insight);
        }
        
        Ok(())
    }

    /// 現在のエピソードのクローンを取得します
    pub fn current_episode(&self) -> Option<Episode<S, E>> {
        self.current_episode.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{policy::RandomPolicy, storage::MemoryStorage};
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

    fn create_test_machine() -> Machine<TestState, TestEvent> {
        let mut machine = Machine::new(TestState::Initial);

        machine.add_transition(TestState::Initial, TestEvent::Start, TestState::Processing);
        machine.add_transition(TestState::Processing, TestEvent::Process, TestState::Processing);
        machine.add_transition(TestState::Processing, TestEvent::Finish, TestState::Final);

        machine
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
        assert_eq!(agent.config.name, "Rustate Agent");
        assert_eq!(agent.machine.current_state(), &TestState::Initial);
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
        agent.start_episode("テストエピソード", Some(TestState::Final)).await.unwrap();
        
        assert!(agent.current_episode.is_some());
        let episode = agent.current_episode.as_ref().unwrap();
        assert_eq!(episode.name, "テストエピソード");
        assert_eq!(episode.initial_state, TestState::Initial);
        assert_eq!(episode.goal_state, Some(TestState::Final));
    }

    #[tokio::test]
    async fn test_agent_decision_and_apply() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![TestEvent::Start]);
        let storage = MemoryStorage::new();

        let mut agent = Agent::new(machine, policy, storage);
        
        // 決定を取得して適用
        let decision = agent.next_decision().await.unwrap();
        assert_eq!(decision.event, TestEvent::Start);
        
        let next_state = agent.apply_decision(&decision).await.unwrap();
        assert_eq!(next_state, TestState::Processing);
        assert_eq!(agent.machine.current_state(), &TestState::Processing);
    }

    #[tokio::test]
    async fn test_agent_step() {
        let machine = create_test_machine();
        let policy = RandomPolicy::new(vec![TestEvent::Start]);
        let storage = MemoryStorage::new();

        let mut agent = Agent::new(machine, policy, storage);
        
        // ステップを実行
        let next_state = agent.step().await.unwrap();
        assert_eq!(next_state, TestState::Processing);
    }
} 