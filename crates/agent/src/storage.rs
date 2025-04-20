use crate::{
    decision::Decision,
    episode::Episode,
    error::AgentError,
    feedback::Feedback,
    insight::Insight,
    observation::Observation,
    prelude::Result,
};
use async_trait::async_trait;
use rustate::{EventTrait, StateTrait};
use serde::{Deserialize, de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use uuid;

/// エージェントの経験（観測、決定、洞察、エピソード）を保存するためのトレイト
#[async_trait]
pub trait Storage<S, E>: Send + Sync
where
    S: StateTrait + DeserializeOwned + Debug + Send + Sync + 'static,
    E: EventTrait + for<'a> Deserialize<'a> + Debug + Clone + Send + Sync + 'static,
{
    /// 観測データを保存します
    async fn save_observation(&self, observation: &Observation<S, E>) -> Result<(), AgentError>;

    /// IDで観測データを取得します
    async fn get_observation(&self, id: &str) -> Result<Observation<S, E>, AgentError>;

    /// 条件に一致する観測データを検索します
    async fn find_observations(
        &self,
        filter: Option<fn(&Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, AgentError>;

    /// 決定を保存します
    async fn save_decision(&self, decision: &Decision<E>) -> Result<(), AgentError>;

    /// IDで決定を取得します
    async fn get_decision(&self, id: &str) -> Result<Decision<E>, AgentError>;

    /// 条件に一致する決定を検索します
    async fn find_decisions(
        &self,
        filter: Option<fn(&Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, AgentError>;

    /// 洞察を保存します
    async fn save_insight(&self, insight: &Insight) -> Result<(), AgentError>;

    /// IDで洞察を取得します
    async fn get_insight(&self, id: &str) -> Result<Insight, AgentError>;

    /// 条件に一致する洞察を検索します
    async fn find_insights(
        &self,
        filter: Option<fn(&Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, AgentError>;

    /// エピソードを保存します
    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), AgentError>;

    /// IDでエピソードを取得します
    async fn get_episode(&self, id: &str) -> Result<Episode<S, E>, AgentError>;

    /// 条件に一致するエピソードを検索します
    async fn find_episodes(
        &self,
        filter: Option<fn(&Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, AgentError>;

    /// フィードバックを保存します
    async fn save_feedback(&self, feedback: &Feedback<E>) -> Result<(), AgentError>;

    /// IDでフィードバックを取得します
    async fn get_feedback(&self, id: &str) -> Result<Feedback<E>, AgentError>;

    /// 条件に一致するフィードバックを検索します
    async fn find_feedback(
        &self,
        filter: Option<fn(&Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, AgentError>;
}

/// ストレージのクエリパラメータ
#[derive(Clone, Debug)]
pub struct StorageQuery {
    /// フィールド名と値のペア
    pub filters: Vec<(String, String)>,
    /// 降順（true）または昇順（false）でソート
    pub sort_descending: bool,
    /// 開始タイムスタンプ（UNIXタイムスタンプ）
    pub from_timestamp: Option<u64>,
    /// 終了タイムスタンプ（UNIXタイムスタンプ）
    pub to_timestamp: Option<u64>,
}

impl StorageQuery {
    /// 新しいクエリを作成します
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            sort_descending: true,
            from_timestamp: None,
            to_timestamp: None,
        }
    }

    /// フィルタを追加します
    pub fn add_filter(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push((field.into(), value.into()));
        self
    }

    /// ソート順を設定します
    pub fn sort_descending(mut self, descending: bool) -> Self {
        self.sort_descending = descending;
        self
    }

    /// 開始タイムスタンプを設定します
    pub fn from_timestamp(mut self, timestamp: u64) -> Self {
        self.from_timestamp = Some(timestamp);
        self
    }

    /// 終了タイムスタンプを設定します
    pub fn to_timestamp(mut self, timestamp: u64) -> Self {
        self.to_timestamp = Some(timestamp);
        self
    }
}

/// ストレージに対するフィルター条件
#[derive(Debug, Clone)]
pub struct StorageFilter {
    /// フィルターのキー
    pub key: String,
    /// フィルターの値
    pub value: String,
    /// フィルターの演算子
    pub operator: FilterOperator,
}

/// フィルター演算子の種類
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOperator {
    /// 等しい
    Equal,
    /// 含む
    Contains,
    /// より大きい
    GreaterThan,
    /// より小さい
    LessThan,
    /// 開始位置が一致
    StartsWith,
    /// 終了位置が一致
    EndsWith,
}

/// インメモリストレージの実装
pub struct MemoryStorage<S, E>
where
    S: StateTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
    E: EventTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
{
    observations: Arc<Mutex<Vec<Observation<S, E>>>>,
    decisions: Arc<Mutex<Vec<Decision<E>>>>,
    insights: Arc<Mutex<Vec<Insight>>>,
    episodes: Arc<Mutex<Vec<Episode<S, E>>>>,
    feedback: Arc<Mutex<Vec<Feedback<E>>>>,
}

impl<S, E> MemoryStorage<S, E>
where
    S: StateTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
    E: EventTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
{
    /// 新しいメモリベースのストレージを作成します
    pub fn new() -> Self {
        Self {
            observations: Arc::new(Mutex::new(Vec::new())),
            decisions: Arc::new(Mutex::new(Vec::new())),
            insights: Arc::new(Mutex::new(Vec::new())),
            episodes: Arc::new(Mutex::new(Vec::new())),
            feedback: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl<S, E> Storage<S, E> for MemoryStorage<S, E>
where
    S: StateTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
    E: EventTrait + Clone + Debug + Send + Sync + for<'a> Deserialize<'a> + 'static,
{
    async fn save_observation(&self, observation: &Observation<S, E>) -> Result<(), AgentError> {
        let mut observations = self.observations.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        observations.push(observation.clone());
        Ok(())
    }

    async fn get_observation(&self, id: &str) -> Result<Observation<S, E>, AgentError> {
        let observations = self.observations.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        observations
            .iter()
            .find(|obs| obs.id == id)
            .cloned()
            .ok_or_else(|| AgentError::StorageError(format!("観測 ID {} が見つかりません", id)))
    }

    async fn find_observations(
        &self,
        filter: Option<fn(&Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, AgentError> {
        let observations = self.observations.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;

        let mut result: Vec<Observation<S, E>> = observations.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(|obs| {
                filter_fn(obs)
            }).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_decision(&self, decision: &Decision<E>) -> Result<(), AgentError> {
        let mut decisions = self.decisions.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        decisions.push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, id: &str) -> Result<Decision<E>, AgentError> {
        let decisions = self.decisions.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        decisions
            .iter()
            .find(|dec| dec.id == id)
            .cloned()
            .ok_or_else(|| AgentError::StorageError(format!("決定 ID {} が見つかりません", id)))
    }

    async fn find_decisions(
        &self,
        filter: Option<fn(&Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, AgentError> {
        let decisions = self.decisions.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;

        let mut result: Vec<Decision<E>> = decisions.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(|dec| {
                filter_fn(dec)
            }).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_insight(&self, insight: &Insight) -> Result<(), AgentError> {
        let mut insights = self.insights.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        insights.push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, id: &str) -> Result<Insight, AgentError> {
        let insights = self.insights.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        insights
            .iter()
            .find(|ins| ins.id == id)
            .cloned()
            .ok_or_else(|| AgentError::StorageError(format!("洞察 ID {} が見つかりません", id)))
    }

    async fn find_insights(
        &self,
        filter: Option<fn(&Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, AgentError> {
        let insights = self.insights.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;

        let mut result: Vec<Insight> = insights.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(|ins| {
                filter_fn(ins)
            }).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), AgentError> {
        let mut episodes = self.episodes.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        episodes.push(episode.clone());
        Ok(())
    }

    async fn get_episode(&self, id: &str) -> Result<Episode<S, E>, AgentError> {
        let episodes = self.episodes.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        
        // Try to parse the ID string as UUID
        let uuid = uuid::Uuid::parse_str(id).map_err(|_| {
            AgentError::StorageError(format!("無効なUUID形式: {}", id))
        })?;
        
        episodes
            .iter()
            .find(|ep| ep.id == uuid)
            .cloned()
            .ok_or_else(|| AgentError::StorageError(format!("エピソード ID {} が見つかりません", id)))
    }

    async fn find_episodes(
        &self,
        filter: Option<fn(&Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, AgentError> {
        let episodes = self.episodes.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;

        let mut result: Vec<Episode<S, E>> = episodes.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(|ep| {
                filter_fn(ep)
            }).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_feedback(&self, feedback: &Feedback<E>) -> Result<(), AgentError> {
        let mut feedbacks = self.feedback.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        feedbacks.push(feedback.clone());
        Ok(())
    }
    
    async fn get_feedback(&self, id: &str) -> Result<Feedback<E>, AgentError> {
        let feedbacks = self.feedback.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        feedbacks
            .iter()
            .find(|fb| fb.id == id)
            .cloned()
            .ok_or_else(|| AgentError::StorageError(format!("フィードバック ID {} が見つかりません", id)))
    }
    
    async fn find_feedback(
        &self,
        filter: Option<fn(&Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, AgentError> {
        let feedback = self.feedback.lock().map_err(|e| {
            AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;

        let mut result: Vec<Feedback<E>> = feedback.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(|fb| {
                filter_fn(fb)
            }).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::Decision;
    use crate::feedback::{Feedback, FeedbackType};
    use rustate::{EventTrait, StateTrait, StateType};
    use serde::{Serialize, Deserialize};
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
            // Use a static StateType as this is just for tests
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

    #[tokio::test]
    async fn test_memory_storage_observations() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();

        let obs1 = Observation::new(
            TestState::Initial,
            TestEvent::Start,
            TestState::Processing,
        );

        let obs2 = Observation::new(
            TestState::Processing,
            TestEvent::Process,
            TestState::Processing,
        );

        // 観測データを保存
        storage.save_observation(&obs1).await.unwrap();
        storage.save_observation(&obs2).await.unwrap();

        // 観測データを取得
        let retrieved = storage.get_observation(&obs1.id).await.unwrap();
        assert_eq!(retrieved.id, obs1.id);

        // 観測データを検索
        let all_obs = storage.find_observations(None, None).await.unwrap();
        assert_eq!(all_obs.len(), 2);
    }

    #[tokio::test]
    async fn test_decision_storage() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let decision = Decision::new(TestEvent::Start, 0.9);
        
        storage.save_decision(&decision).await.unwrap();
        
        let retrieved = storage.get_decision(&decision.id).await.unwrap();
        assert_eq!(retrieved.id, decision.id);
    }

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        
        // Create and store a decision
        let decision = Decision::new(TestEvent::Start, 0.9);
        storage.save_decision(&decision).await.unwrap();
        
        // Retrieve and verify
        let decisions = storage.find_decisions(None, None).await.unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].event, TestEvent::Start);
        
        // Create and store feedback
        let feedback = Feedback::new("良いスタート", FeedbackType::Positive, "システム");
        storage.save_feedback(&feedback).await.unwrap();
        
        // Retrieve and verify
        let feedback_list = storage.find_feedback(None, None).await.unwrap();
        assert_eq!(feedback_list.len(), 1);
        assert_eq!(feedback_list[0].content, "良いスタート");
    }
} 