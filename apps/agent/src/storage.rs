use crate::{decision::Decision, episode::Episode, observation::Observation};
use async_trait::async_trait;
use rustate::{Event, EventTrait, IntoEvent, State, StateTrait};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use crate::error::StorageError;
use crate::feedback::Feedback;
use crate::insight::Insight;

/// エージェントの経験（観測、決定、洞察、エピソード）を保存するためのトレイト
#[async_trait]
pub trait Storage<S, E>: Send + Sync
where
    S: StateTrait + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + Clone,
    E: EventTrait + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + Clone,
{
    /// 観測データを保存します
    async fn save_observation(&self, observation: &Observation<S, E>) -> Result<(), StorageError>;

    /// IDで観測データを取得します
    async fn get_observation(&self, id: &str) -> Result<Option<Observation<S, E>>, StorageError>;

    /// 条件に一致する観測データを検索します
    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError>;

    /// 決定を保存します
    async fn save_decision(&self, decision: &Decision<E>) -> Result<(), StorageError>;

    /// IDで決定を取得します
    async fn get_decision(&self, id: &str) -> Result<Option<Decision<E>>, StorageError>;

    /// 条件に一致する決定を検索します
    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError>;

    /// 洞察を保存します
    async fn save_insight(&self, insight: &Insight) -> Result<(), StorageError>;

    /// IDで洞察を取得します
    async fn get_insight(&self, id: &str) -> Result<Option<Insight>, StorageError>;

    /// 条件に一致する洞察を検索します
    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError>;

    /// エピソードを保存します
    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), StorageError>;

    /// IDでエピソードを取得します
    async fn get_episode(&self, id: &str) -> Result<Option<Episode<S, E>>, StorageError>;

    /// 条件に一致するエピソードを検索します
    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError>;

    /// フィードバックを保存します
    async fn save_feedback(&self, feedback: &Feedback<E>) -> Result<(), StorageError>;

    /// IDでフィードバックを取得します
    async fn get_feedback(&self, id: &str) -> Result<Option<Feedback<E>>, StorageError>;

    /// 条件に一致するフィードバックを検索します
    async fn find_feedback(
        &self,
        filter: Option<for<'a> fn(&'a Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, StorageError>;

    /// 特定のエピソードIDを持つエピソードを取得します。
    async fn get_all_episodes(&self) -> Result<Vec<Episode<S, E>>, StorageError>;

    /// 特定の観測IDを持つ観測を取得します。
    async fn get_observations_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError>;

    /// 特定の決定IDを持つ決定を取得します。
    async fn get_decisions_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Decision<E>>, StorageError>;

    /// 特定の洞察IDを持つ洞察を取得します。
    async fn get_insights_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Insight>, StorageError>;

    /// 特定のフィードバックIDを持つフィードバックを取得します。
    async fn get_feedbacks_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Feedback<E>>, StorageError>;
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

impl Default for StorageQuery {
    fn default() -> Self {
        Self::new()
    }
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
    S: StateTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
{
    observations: Arc<Mutex<Vec<Observation<S, E>>>>,
    decisions: Arc<Mutex<Vec<Decision<E>>>>,
    insights: Arc<Mutex<Vec<Insight>>>,
    episodes: Arc<Mutex<Vec<Episode<S, E>>>>,
    feedback: Arc<Mutex<Vec<Feedback<E>>>>,
}

impl<S, E> Default for MemoryStorage<S, E>
where
    S: StateTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S, E> MemoryStorage<S, E>
where
    S: StateTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
    E: EventTrait
        + Clone
        + Debug
        + Send
        + Sync
        + Serialize
        + for<'deserialize> Deserialize<'deserialize>
        + 'static,
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
    S: StateTrait + Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
    E: EventTrait + Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static,
{
    async fn save_observation(
        &self,
        observation: &Observation<S, E>,
    ) -> std::result::Result<(), StorageError> {
        let mut observations = self
            .observations
            .lock()
            .map_err(|_| StorageError::MutexPoisoned)?;
        observations.push(observation.clone());
        Ok(())
    }

    async fn get_observation(&self, id: &str) -> Result<Option<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;
        Ok(observations.iter().find(|obs| obs.id == id).cloned())
    }

    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Observation<S, E>> = observations.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_decision(&self, decision: &Decision<E>) -> std::result::Result<(), StorageError> {
        let mut decisions = self
            .decisions
            .lock()
            .map_err(|_| StorageError::MutexPoisoned)?;
        decisions.push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, id: &str) -> Result<Option<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;
        Ok(decisions.iter().find(|dec| dec.id == id).cloned())
    }

    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Decision<E>> = decisions.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_insight(&self, insight: &Insight) -> std::result::Result<(), StorageError> {
        let mut insights = self
            .insights
            .lock()
            .map_err(|_| StorageError::MutexPoisoned)?;
        insights.push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, id: &str) -> Result<Option<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;
        Ok(insights.iter().find(|ins| ins.id == id).cloned())
    }

    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Insight> = insights.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> std::result::Result<(), StorageError> {
        let mut episodes = self
            .episodes
            .lock()
            .map_err(|_| StorageError::MutexPoisoned)?;
        episodes.push(episode.clone());
        Ok(())
    }

    async fn get_episode(&self, id: &str) -> Result<Option<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        Ok(episodes
            .iter()
            .find(|episode| episode.id.to_string() == id)
            .cloned())
    }

    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Episode<S, E>> = episodes.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_feedback(&self, feedback: &Feedback<E>) -> std::result::Result<(), StorageError> {
        let mut feedbacks = self
            .feedbacks
            .lock()
            .map_err(|_| StorageError::MutexPoisoned)?;
        feedbacks.push(feedback.clone());
        Ok(())
    }

    async fn get_feedback(&self, id: &str) -> Result<Option<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedbacks
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;
        Ok(feedbacks.iter().find(|f| f.id == id).cloned())
    }

    async fn find_feedback(
        &self,
        filter: Option<for<'a> fn(&'a Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedbacks
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Feedback<E>> = feedbacks.clone();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn get_all_episodes(&self) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;
        Ok(episodes.clone())
    }

    async fn get_observations_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Observation<S, E>> = Vec::new();

        for episode in episodes.iter() {
            if episode.id.to_string() == episode_id {
                result = episode.observations.clone();
                break;
            }
        }

        Ok(result)
    }

    async fn get_decisions_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Decision<E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Decision<E>> = Vec::new();

        for episode in episodes.iter() {
            if episode.id.to_string() == episode_id {
                result = episode.decisions.clone();
                break;
            }
        }

        Ok(result)
    }

    async fn get_insights_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Insight>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Insight> = Vec::new();

        for episode in episodes.iter() {
            if episode.id.to_string() == episode_id {
                result = episode.insights.clone();
                break;
            }
        }

        Ok(result)
    }

    async fn get_feedbacks_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Feedback<E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|e| StorageError::MutexPoisoned(format!("ロック取得エラー: {}", e)))?;

        let mut result: Vec<Feedback<E>> = Vec::new();

        for episode in episodes.iter() {
            if episode.id.to_string() == episode_id {
                result = episode.feedbacks.clone();
                break;
            }
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
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    enum TestState {
        Initial,
        Processing,
        Final,
    }

    impl Display for TestState {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                TestState::Initial => write!(f, "Initial"),
                TestState::Processing => write!(f, "Processing"),
                TestState::Final => write!(f, "Final"),
            }
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &Self {
            self
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

        fn name(&self) -> &str {
            match self {
                TestEvent::Start => "START",
                TestEvent::Process => "PROCESS",
                TestEvent::Finish => "FINISH",
            }
        }
    }

    #[tokio::test]
    async fn test_memory_storage_observations() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();

        let obs1 = Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);

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
        let decision = Decision::new(
            "decision_test_1",
            TestEvent::Start,
            0.9,
            None::<TestState>,
            None::<TestState>,
        );

        storage.save_decision(&decision).await.unwrap();

        let retrieved = storage.get_decision(&decision.id).await.unwrap();
        assert_eq!(retrieved.id, decision.id);
    }

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();

        // Create and store a decision
        let decision = Decision::new(
            "memory_test_decision_1",
            TestEvent::Start,
            0.9,
            None::<TestState>,
            None::<TestState>,
        );
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
