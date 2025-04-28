use crate::agent::AgentId;
use crate::decision::Decision;
use crate::episode::Episode;
use crate::error::{self, Result as AgentResult, StorageError};
use crate::feedback::{Feedback, FeedbackType};
use crate::insight::Insight;
use crate::observation::Observation;
use async_trait::async_trait;
use futures_util::TryFutureExt;
use rustate::{EventTrait, StateTrait};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;

/// エージェントの経験（観測、決定、洞察、エピソード）を保存するためのトレイト
#[async_trait]
pub trait Storage<S, E>: Send + Sync
where
    S: StateTrait + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + Clone,
    E: EventTrait + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + Clone,
{
    /// 観測データを保存します
    async fn save_observation(
        &self,
        episode_id: &str,
        observation: &Observation<S, E>,
    ) -> Result<(), StorageError>;

    /// IDで観測データを取得します
    async fn get_observation(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError>;

    /// 条件に一致する観測データを検索します
    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError>;

    /// 決定を保存します
    async fn save_decision(
        &self,
        episode_id: &str,
        decision: &Decision<E>,
    ) -> Result<(), StorageError>;

    /// IDで決定を取得します
    async fn get_decision(&self, episode_id: &str) -> Result<Vec<Decision<E>>, StorageError>;

    /// 条件に一致する決定を検索します
    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError>;

    /// 洞察を保存します
    async fn save_insight(&self, episode_id: &str, insight: &Insight) -> Result<(), StorageError>;

    /// IDで洞察を取得します
    async fn get_insight(&self, episode_id: &str) -> Result<Vec<Insight>, StorageError>;

    /// 条件に一致する洞察を検索します
    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError>;

    /// エピソードを保存します
    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), StorageError>;

    /// IDでエピソードを取得します
    async fn get_episode(&self, episode_id: &str) -> Result<Option<Episode<S, E>>, StorageError>;

    /// 条件に一致するエピソードを検索します
    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError>;

    /// フィードバックを保存します
    async fn save_feedback(
        &self,
        episode_id: &str,
        feedback: &Feedback<E>,
    ) -> Result<(), StorageError>;

    /// IDでフィードバックを取得します
    async fn get_feedback(&self, episode_id: &str) -> Result<Vec<Feedback<E>>, StorageError>;

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

    /// 特定のエピソードIDを持つエピソードを削除します。
    async fn delete_observation(
        &self,
        episode_id: &str,
        observation_id: &str,
    ) -> Result<(), StorageError>;

    /// 特定のエピソードIDを持つ決定を削除します。
    async fn delete_decision(
        &self,
        episode_id: &str,
        decision_id: &str,
    ) -> Result<(), StorageError>;

    /// 特定のエピソードIDを持つ洞察を削除します。
    async fn delete_insight(&self, episode_id: &str, insight_id: &str) -> Result<(), StorageError>;

    /// 特定のエピソードIDを持つエピソードを削除します。
    async fn delete_episode(&self, episode_id: &str) -> Result<(), StorageError>;

    /// 特定のエピソードIDを持つフィードバックを削除します。
    async fn delete_feedback(
        &self,
        episode_id: &str,
        feedback_id: &str,
    ) -> Result<(), StorageError>;
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
    observations: Arc<Mutex<HashMap<String, Vec<Observation<S, E>>>>>,
    decisions: Arc<Mutex<HashMap<String, Vec<Decision<E>>>>>,
    insights: Arc<Mutex<HashMap<String, Vec<Insight>>>>,
    episodes: Arc<Mutex<HashMap<String, Episode<S, E>>>>,
    feedback: Arc<Mutex<HashMap<String, Vec<Feedback<E>>>>>,
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
            observations: Arc::new(Mutex::new(HashMap::new())),
            decisions: Arc::new(Mutex::new(HashMap::new())),
            insights: Arc::new(Mutex::new(HashMap::new())),
            episodes: Arc::new(Mutex::new(HashMap::new())),
            feedback: Arc::new(Mutex::new(HashMap::new())),
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
        episode_id: &str,
        observation: &Observation<S, E>,
    ) -> Result<(), StorageError> {
        let mut observations = self.observations.lock().await;
        observations
            .entry(episode_id.to_string())
            .or_default()
            .push(observation.clone());
        Ok(())
    }

    async fn get_observation(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self.observations.lock().await;
        Ok(observations.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self.observations.lock().await;

        let mut result: Vec<Observation<S, E>> = observations
            .values()
            .flat_map(|v| v.iter())
            .cloned()
            .collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_decision(
        &self,
        episode_id: &str,
        decision: &Decision<E>,
    ) -> Result<(), StorageError> {
        let mut decisions = self.decisions.lock().await;
        decisions
            .entry(episode_id.to_string())
            .or_default()
            .push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, episode_id: &str) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self.decisions.lock().await;
        Ok(decisions.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self.decisions.lock().await;

        let mut result: Vec<Decision<E>> =
            decisions.values().flat_map(|v| v.iter()).cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_insight(&self, episode_id: &str, insight: &Insight) -> Result<(), StorageError> {
        let mut insights = self.insights.lock().await;
        insights
            .entry(episode_id.to_string())
            .or_default()
            .push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, episode_id: &str) -> Result<Vec<Insight>, StorageError> {
        let insights = self.insights.lock().await;
        Ok(insights.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError> {
        let insights = self.insights.lock().await;

        let mut result: Vec<Insight> = insights.values().flat_map(|v| v.iter()).cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), StorageError> {
        let mut episodes = self.episodes.lock().await;
        episodes
            .entry(episode.id.to_string())
            .or_insert_with(|| episode.clone());
        Ok(())
    }

    async fn get_episode(&self, episode_id: &str) -> Result<Option<Episode<S, E>>, StorageError> {
        let episodes = self.episodes.lock().await;
        Ok(episodes.get(episode_id).cloned())
    }

    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self.episodes.lock().await;

        let mut result: Vec<Episode<S, E>> = episodes.values().cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_feedback(
        &self,
        episode_id: &str,
        feedback: &Feedback<E>,
    ) -> Result<(), StorageError> {
        let mut feedbacks = self.feedback.lock().await;
        feedbacks
            .entry(episode_id.to_string())
            .or_default()
            .push(feedback.clone());
        Ok(())
    }

    async fn get_feedback(&self, episode_id: &str) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self.feedback.lock().await;
        Ok(feedbacks.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_feedback(
        &self,
        filter: Option<for<'a> fn(&'a Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self.feedback.lock().await;

        let mut result: Vec<Feedback<E>> =
            feedbacks.values().flat_map(|v| v.iter()).cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn get_all_episodes(&self) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self.episodes.lock().await;
        Ok(episodes.values().cloned().collect())
    }

    async fn get_observations_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let episodes = self.episodes.lock().await;

        let mut result: Vec<Observation<S, E>> = Vec::new();

        for episode in episodes.values() {
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
        let episodes = self.episodes.lock().await;

        let mut result: Vec<Decision<E>> = Vec::new();

        for episode in episodes.values() {
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
        let episodes = self.episodes.lock().await;

        let mut result: Vec<Insight> = Vec::new();

        for episode in episodes.values() {
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
        let episodes = self.episodes.lock().await;

        let mut result: Vec<Feedback<E>> = Vec::new();

        for episode in episodes.values() {
            if episode.id.to_string() == episode_id {
                if let Some(fb) = &episode.feedback {
                    result.push(fb.clone());
                }
                break;
            }
        }

        Ok(result)
    }

    async fn delete_observation(
        &self,
        episode_id: &str,
        observation_id: &str,
    ) -> Result<(), StorageError> {
        let mut observations = self.observations.lock().await;
        if let Some(obs_list) = observations.get_mut(episode_id) {
            obs_list.retain(|obs| obs.id != observation_id);
        }
        Ok(())
    }

    async fn delete_decision(
        &self,
        episode_id: &str,
        decision_id: &str,
    ) -> Result<(), StorageError> {
        let mut decisions = self.decisions.lock().await;
        if let Some(dec_list) = decisions.get_mut(episode_id) {
            dec_list.retain(|dec| dec.id != decision_id);
        }
        Ok(())
    }

    async fn delete_insight(&self, episode_id: &str, insight_id: &str) -> Result<(), StorageError> {
        let mut insights = self.insights.lock().await;
        if let Some(ins_list) = insights.get_mut(episode_id) {
            ins_list.retain(|ins| ins.id != insight_id);
        }
        Ok(())
    }

    async fn delete_episode(&self, episode_id: &str) -> Result<(), StorageError> {
        let mut episodes = self.episodes.lock().await;
        episodes.remove(episode_id);
        self.observations.lock().await.remove(episode_id);
        self.decisions.lock().await.remove(episode_id);
        self.insights.lock().await.remove(episode_id);
        self.feedback.lock().await.remove(episode_id);
        Ok(())
    }

    async fn delete_feedback(
        &self,
        episode_id: &str,
        feedback_id: &str,
    ) -> Result<(), StorageError> {
        let mut feedbacks = self.feedback.lock().await;
        if let Some(fb_list) = feedbacks.get_mut(episode_id) {
            fb_list.retain(|fb| fb.id != feedback_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decision::Decision,
        episode::Episode,
        error::StorageError,
        feedback::{Feedback, FeedbackType},
        goal::Goal,
        insight::InsightType,
        observation::Observation,
    };
    use rustate::{EventTrait, StateTrait};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        sync::Arc,
        time::SystemTime,
    };
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    enum TestState {
        Initial,
        Processing,
        Final,
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

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    fn create_test_episode(id: &str) -> Episode<TestState, TestEvent> {
        Episode {
            id: Uuid::parse_str(id).unwrap_or_else(|_| Uuid::new_v4()),
            name: format!("Episode {}", id),
            start_time: SystemTime::now(),
            observations: Vec::new(),
            decisions: Vec::new(),
            insights: Vec::new(),
            feedback: None,
            end_time: None,
            initial_state: TestState::Initial,
            goal: crate::goal::Goal::new(TestState::Final),
            metadata: Value::Null,
            is_successful: false,
            overall_score: 0.0,
        }
    }

    #[tokio::test]
    async fn test_save_and_get_observation() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let observation =
            Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);

        storage
            .save_observation(episode_id, &observation)
            .await
            .unwrap();

        let observations = storage.get_observation(episode_id).await.unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].previous_state, observation.previous_state);
        assert_eq!(observations[0].event, observation.event);
        assert_eq!(observations[0].next_state, observation.next_state);
    }

    #[tokio::test]
    async fn test_save_and_get_decision() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let decision = Decision::new(
            Uuid::new_v4().to_string(),
            TestEvent::Process,
            0.9,
            Some("Mock decision".to_string()),
            Some(TestState::Initial),
            Some(TestState::Processing),
            Some(TestState::Final),
        );

        storage.save_decision(episode_id, &decision).await.unwrap();

        let decisions = storage.get_decision(episode_id).await.unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].id, decision.id);
        assert_eq!(decisions[0].event, decision.event);
    }

    #[tokio::test]
    async fn test_save_and_get_insight() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let insight = Insight::new(
            Uuid::new_v4().to_string(),
            "Test insight".to_string(),
            InsightType::General,
            Some(1.0),
            None,
            None,
        );

        storage.save_insight(episode_id, &insight).await.unwrap();

        let insights = storage.get_insight(episode_id).await.unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].id, insight.id);
        assert_eq!(insights[0].content, insight.content);
    }

    #[tokio::test]
    async fn test_save_and_get_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode = create_test_episode("test_episode");

        storage.save_episode(&episode).await.unwrap();

        let retrieved_episode = storage.get_episode(&episode.id.to_string()).await.unwrap();
        assert!(retrieved_episode.is_some());
        assert_eq!(retrieved_episode.unwrap().id, episode.id);
    }

    #[tokio::test]
    async fn test_save_and_get_feedback() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let feedback = Feedback::new(
            Uuid::new_v4().to_string(),
            FeedbackType::Positive,
            Some("Good job!".to_string()),
            Some("user".to_string()),
            None,
            Some(1.0),
        );

        storage.save_feedback(episode_id, &feedback).await.unwrap();

        let feedbacks = storage.get_feedback(episode_id).await.unwrap();
        assert_eq!(feedbacks.len(), 1);
        assert_eq!(feedbacks[0].id, feedback.id);
        assert_eq!(feedbacks[0].feedback_type, feedback.feedback_type);
    }

    #[tokio::test]
    async fn test_get_all_episodes() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode1 = create_test_episode("episode1");
        let episode2 = create_test_episode("episode2");

        storage.save_episode(&episode1).await.unwrap();
        storage.save_episode(&episode2).await.unwrap();

        let episodes = storage.get_all_episodes().await.unwrap();
        assert_eq!(episodes.len(), 2);
        assert!(episodes.iter().any(|e| e.id == episode1.id));
        assert!(episodes.iter().any(|e| e.id == episode2.id));
    }

    #[tokio::test]
    async fn test_get_observations_for_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let observation1 =
            Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);
        let observation2 =
            Observation::new(TestState::Processing, TestEvent::Process, TestState::Final);

        storage
            .save_observation(episode_id, &observation1)
            .await
            .unwrap();
        storage
            .save_observation(episode_id, &observation2)
            .await
            .unwrap();

        let mut episode = create_test_episode(episode_id);
        episode.observations = vec![observation1.clone(), observation2.clone()];
        storage.save_episode(&episode).await.unwrap();

        let observations = storage
            .get_observations_for_episode(episode_id)
            .await
            .unwrap();
        assert_eq!(observations.len(), 2);
        assert!(observations.iter().any(|o| o.event == observation1.event));
        assert!(observations.iter().any(|o| o.event == observation2.event));
    }

    #[tokio::test]
    async fn test_get_decisions_for_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let decision1 = Decision::new(
            Uuid::new_v4().to_string(),
            TestEvent::Start,
            0.9,
            None,
            Some(TestState::Initial),
            Some(TestState::Processing),
            Some(TestState::Final),
        );
        let decision2 = Decision::new(
            Uuid::new_v4().to_string(),
            TestEvent::Process,
            0.8,
            None,
            Some(TestState::Processing),
            Some(TestState::Processing),
            Some(TestState::Final),
        );

        storage.save_decision(episode_id, &decision1).await.unwrap();
        storage.save_decision(episode_id, &decision2).await.unwrap();

        let mut episode = create_test_episode(episode_id);
        episode.decisions = vec![decision1.clone(), decision2.clone()];
        storage.save_episode(&episode).await.unwrap();

        let decisions = storage.get_decisions_for_episode(episode_id).await.unwrap();
        assert_eq!(decisions.len(), 2);
        assert!(decisions.iter().any(|d| d.id == decision1.id));
        assert!(decisions.iter().any(|d| d.id == decision2.id));
    }

    #[tokio::test]
    async fn test_get_insights_for_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let insight1 = Insight::new(
            Uuid::new_v4().to_string(),
            "Insight 1".to_string(),
            InsightType::General,
            None,
            None,
            None,
        );
        let insight2 = Insight::new(
            Uuid::new_v4().to_string(),
            "Insight 2".to_string(),
            InsightType::Action,
            None,
            None,
            None,
        );

        storage.save_insight(episode_id, &insight1).await.unwrap();
        storage.save_insight(episode_id, &insight2).await.unwrap();

        let mut episode = create_test_episode(episode_id);
        episode.insights = vec![insight1.clone(), insight2.clone()];
        storage.save_episode(&episode).await.unwrap();

        let insights = storage.get_insights_for_episode(episode_id).await.unwrap();
        assert_eq!(insights.len(), 2);
        assert!(insights.iter().any(|i| i.id == insight1.id));
        assert!(insights.iter().any(|i| i.id == insight2.id));
    }

    #[tokio::test]
    async fn test_get_feedbacks_for_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let feedback1 = Feedback::new(
            Uuid::new_v4().to_string(),
            FeedbackType::Positive,
            None,
            None,
            None,
            None,
        );
        let feedback2 = Feedback::new(
            Uuid::new_v4().to_string(),
            FeedbackType::Negative,
            None,
            None,
            None,
            None,
        );

        storage.save_feedback(episode_id, &feedback1).await.unwrap();
        storage.save_feedback(episode_id, &feedback2).await.unwrap();

        let mut episode = create_test_episode(episode_id);
        episode.feedback = Some(feedback1.clone());
        storage.save_episode(&episode).await.unwrap();

        let feedbacks = storage.get_feedbacks_for_episode(episode_id).await.unwrap();
        assert_eq!(feedbacks.len(), 1);
        assert!(feedbacks.iter().any(|f| f.id == feedback1.id));
    }

    #[tokio::test]
    async fn test_delete_observation() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let observation =
            Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);
        let obs_id = observation.id.clone();

        storage
            .save_observation(episode_id, &observation)
            .await
            .unwrap();

        let observations = storage.get_observation(episode_id).await.unwrap();
        assert_eq!(observations.len(), 1);

        storage
            .delete_observation(episode_id, &obs_id.to_string())
            .await
            .unwrap();

        let observations = storage.get_observation(episode_id).await.unwrap();
        assert_eq!(observations.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_decision() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let decision = Decision::new(
            Uuid::new_v4().to_string(),
            TestEvent::Process,
            0.9,
            None,
            Some(TestState::Initial),
            Some(TestState::Processing),
            Some(TestState::Final),
        );
        let dec_id = decision.id.clone();

        storage.save_decision(episode_id, &decision).await.unwrap();

        let decisions = storage.get_decision(episode_id).await.unwrap();
        assert_eq!(decisions.len(), 1);

        storage.delete_decision(episode_id, &dec_id).await.unwrap();

        let decisions = storage.get_decision(episode_id).await.unwrap();
        assert_eq!(decisions.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_insight() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let insight = Insight::new(
            Uuid::new_v4().to_string(),
            "Test insight".to_string(),
            InsightType::General,
            None,
            None,
            None,
        );
        let ins_id = insight.id.clone();

        storage.save_insight(episode_id, &insight).await.unwrap();

        let insights = storage.get_insight(episode_id).await.unwrap();
        assert_eq!(insights.len(), 1);

        storage.delete_insight(episode_id, &ins_id).await.unwrap();

        let insights = storage.get_insight(episode_id).await.unwrap();
        assert_eq!(insights.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_episode() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode = create_test_episode("test_episode");
        let episode_id_str = episode.id.to_string();

        storage.save_episode(&episode).await.unwrap();

        let retrieved_episode = storage.get_episode(&episode_id_str).await.unwrap();
        assert!(retrieved_episode.is_some());

        storage.delete_episode(&episode_id_str).await.unwrap();

        let retrieved_episode = storage.get_episode(&episode_id_str).await.unwrap();
        assert!(retrieved_episode.is_none());
    }

    #[tokio::test]
    async fn test_delete_feedback() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "test_episode";
        let feedback = Feedback::new(
            Uuid::new_v4().to_string(),
            FeedbackType::Positive,
            None,
            None,
            None,
            None,
        );
        let fb_id = feedback.id.clone();

        storage.save_feedback(episode_id, &feedback).await.unwrap();

        let feedbacks = storage.get_feedback(episode_id).await.unwrap();
        assert_eq!(feedbacks.len(), 1);

        storage.delete_feedback(episode_id, &fb_id).await.unwrap();

        let feedbacks = storage.get_feedback(episode_id).await.unwrap();
        assert_eq!(feedbacks.len(), 0);
    }
}
