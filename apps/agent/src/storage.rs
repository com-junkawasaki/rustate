use crate::agent::AgentId;
use crate::decision::Decision;
use crate::episode::Episode;
use crate::error::{self, Result as AgentResult, StorageError};
use crate::feedback::{Feedback, FeedbackType};
use crate::insight::Insight;
use crate::observation::Observation;
use rustate::{EventTrait, StateTrait};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::TryFutureExt;
use std::collections::HashMap;

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
    async fn get_observation(&self, episode_id: &str) -> Result<Vec<Observation<S, E>>, StorageError>;

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
    async fn save_insight(
        &self,
        episode_id: &str,
        insight: &Insight,
    ) -> Result<(), StorageError>;

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
    async fn delete_insight(
        &self,
        episode_id: &str,
        insight_id: &str,
    ) -> Result<(), StorageError>;

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
    episodes: Arc<Mutex<HashMap<String, Episode<S, E>>>>>,
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
        let mut observations = self
            .observations
            .lock()
            .await;
        observations
            .entry(episode_id.to_string())
            .or_default()
            .push(observation.clone());
        Ok(())
    }

    async fn get_observation(&self, episode_id: &str) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .await;
        Ok(observations
            .get(episode_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .await;

        let mut result: Vec<Observation<S, E>> = observations.values().cloned().collect();

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
        let mut decisions = self
            .decisions
            .lock()
            .await;
        decisions
            .entry(episode_id.to_string())
            .or_default()
            .push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, episode_id: &str) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .await;
        Ok(decisions.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .await;

        let mut result: Vec<Decision<E>> = decisions.values().cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_insight(
        &self,
        episode_id: &str,
        insight: &Insight,
    ) -> Result<(), StorageError> {
        let mut insights = self
            .insights
            .lock()
            .await;
        insights
            .entry(episode_id.to_string())
            .or_default()
            .push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, episode_id: &str) -> Result<Vec<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .await;
        Ok(insights.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .await;

        let mut result: Vec<Insight> = insights.values().cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), StorageError> {
        let mut episodes = self
            .episodes
            .lock()
            .await;
        episodes
            .entry(episode.id.to_string())
            .or_insert_with(|| episode.clone());
        Ok(())
    }

    async fn get_episode(&self, episode_id: &str) -> Result<Option<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;
        Ok(episodes.get(episode_id).cloned())
    }

    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let mut feedbacks = self
            .feedback
            .lock()
            .await;
        feedbacks
            .entry(episode_id.to_string())
            .or_default()
            .push(feedback.clone());
        Ok(())
    }

    async fn get_feedback(&self, episode_id: &str) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedback
            .lock()
            .await;
        Ok(feedbacks.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_feedback(
        &self,
        filter: Option<for<'a> fn(&'a Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedback
            .lock()
            .await;

        let mut result: Vec<Feedback<E>> = feedbacks.values().cloned().collect();

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
            .await;
        Ok(episodes.values().cloned().collect())
    }

    async fn get_observations_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

        let mut result: Vec<Feedback<E>> = Vec::new();

        for episode in episodes.values() {
            if episode.id.to_string() == episode_id {
                result = episode.feedbacks.clone();
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

    async fn delete_insight(
        &self,
        episode_id: &str,
        insight_id: &str,
    ) -> Result<(), StorageError> {
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
    use crate::decision::Decision;
    use crate::episode::Episode;
    use crate::feedback::{Feedback, FeedbackType};
    use crate::observation::Observation;
    use rustate::{StateTrait};
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Display, Formatter};
    use serde_json::Value;
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
use crate::agent::AgentId;
use crate::decision::Decision;
use crate::episode::Episode;
use crate::error::{self, Result as AgentResult, StorageError};
use crate::feedback::{Feedback, FeedbackType};
use crate::insight::Insight;
use crate::observation::Observation;
use rustate::{EventTrait, StateTrait};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::TryFutureExt;
use std::collections::HashMap;

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
    async fn get_observation(&self, episode_id: &str) -> Result<Vec<Observation<S, E>>, StorageError>;

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
    async fn save_insight(
        &self,
        episode_id: &str,
        insight: &Insight,
    ) -> Result<(), StorageError>;

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
    async fn delete_insight(
        &self,
        episode_id: &str,
        insight_id: &str,
    ) -> Result<(), StorageError>;

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
    episodes: Arc<Mutex<HashMap<String, Episode<S, E>>>>>,
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
        let mut observations = self
            .observations
            .lock()
            .await;
        observations
            .entry(episode_id.to_string())
            .or_default()
            .push(observation.clone());
        Ok(())
    }

    async fn get_observation(&self, episode_id: &str) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .await;
        Ok(observations
            .get(episode_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn find_observations(
        &self,
        filter: Option<for<'a> fn(&'a Observation<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let observations = self
            .observations
            .lock()
            .await;

        let mut result: Vec<Observation<S, E>> = observations.values().cloned().collect();

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
        let mut decisions = self
            .decisions
            .lock()
            .await;
        decisions
            .entry(episode_id.to_string())
            .or_default()
            .push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, episode_id: &str) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .await;
        Ok(decisions.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_decisions(
        &self,
        filter: Option<for<'a> fn(&'a Decision<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>, StorageError> {
        let decisions = self
            .decisions
            .lock()
            .await;

        let mut result: Vec<Decision<E>> = decisions.values().cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_insight(
        &self,
        episode_id: &str,
        insight: &Insight,
    ) -> Result<(), StorageError> {
        let mut insights = self
            .insights
            .lock()
            .await;
        insights
            .entry(episode_id.to_string())
            .or_default()
            .push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, episode_id: &str) -> Result<Vec<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .await;
        Ok(insights.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_insights(
        &self,
        filter: Option<for<'a> fn(&'a Insight) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>, StorageError> {
        let insights = self
            .insights
            .lock()
            .await;

        let mut result: Vec<Insight> = insights.values().cloned().collect();

        if let Some(filter_fn) = filter {
            result = result.into_iter().filter(filter_fn).collect();
        }

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<(), StorageError> {
        let mut episodes = self
            .episodes
            .lock()
            .await;
        episodes
            .entry(episode.id.to_string())
            .or_insert_with(|| episode.clone());
        Ok(())
    }

    async fn get_episode(&self, episode_id: &str) -> Result<Option<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;
        Ok(episodes.get(episode_id).cloned())
    }

    async fn find_episodes(
        &self,
        filter: Option<for<'a> fn(&'a Episode<S, E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let mut feedbacks = self
            .feedback
            .lock()
            .await;
        feedbacks
            .entry(episode_id.to_string())
            .or_default()
            .push(feedback.clone());
        Ok(())
    }

    async fn get_feedback(&self, episode_id: &str) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedback
            .lock()
            .await;
        Ok(feedbacks.get(episode_id).cloned().unwrap_or_default())
    }

    async fn find_feedback(
        &self,
        filter: Option<for<'a> fn(&'a Feedback<E>) -> bool>,
        limit: Option<usize>,
    ) -> Result<Vec<Feedback<E>>, StorageError> {
        let feedbacks = self
            .feedback
            .lock()
            .await;

        let mut result: Vec<Feedback<E>> = feedbacks.values().cloned().collect();

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
            .await;
        Ok(episodes.values().cloned().collect())
    }

    async fn get_observations_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Vec<Observation<S, E>>, StorageError> {
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

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
        let episodes = self
            .episodes
            .lock()
            .await;

        let mut result: Vec<Feedback<E>> = Vec::new();

        for episode in episodes.values() {
            if episode.id.to_string() == episode_id {
                result = episode.feedbacks.clone();
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

    async fn delete_insight(
        &self,
        episode_id: &str,
        insight_id: &str,
    ) -> Result<(), StorageError> {
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
    use crate::decision::Decision;
    use crate::episode::Episode;
    use crate::feedback::{Feedback, FeedbackType};
    use crate::observation::Observation;
    use rustate::{StateTrait};
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Display, Formatter};

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

        fn payload(&self) -> Option<&serde_json::Value> {
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
            id: id.parse().unwrap(),
            name: format!("Episode {}", id),
            start_time: SystemTime::now(),
            end_time: None,
            initial_state: TestState::Initial,
            goal: crate::goal::Goal::new(TestState::Final),
            observations: Vec::new(),
            decisions: Vec::new(),
            insights: Vec::new(),
            metadata: serde_json::Value::Null,
            is_successful: false,
            overall_score: 0.0,
            feedback: None,
        }
    }

    #[tokio::test]
    async fn test_memory_storage_observations() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "ep1";
        let obs1 = Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);
        let obs2 = Observation::new(
            TestState::Processing,
            TestEvent::Process,
            TestState::Processing,
        );

        storage.save_observation(episode_id, &obs1).await.unwrap();
        storage.save_observation(episode_id, &obs2).await.unwrap();

        let retrieved_obs = storage.get_observation(episode_id).await.unwrap();
        assert_eq!(retrieved_obs.len(), 2);
        assert!(retrieved_obs.contains(&obs1));
        assert!(retrieved_obs.contains(&obs2));
    }

    #[tokio::test]
    async fn test_decision_storage() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "ep1";
        let decision = Decision::new(
            "dec1".to_string(),
            TestEvent::Process,
            0.9,
            Some(TestState::Processing),
            Some(TestState::Final),
        );

        storage.save_decision(episode_id, &decision).await.unwrap();
        let retrieved_decisions = storage.get_decision(episode_id).await.unwrap();
        assert_eq!(retrieved_decisions.len(), 1);
        assert_eq!(retrieved_decisions[0].id, decision.id);
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
        storage.save_decision("ep1", &decision).await.unwrap();

        // Retrieve and verify
        let decisions = storage.find_decisions(None, None).await.unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].event, TestEvent::Start);

        // Create and store feedback
        let feedback = Feedback::new("良いスタート", FeedbackType::Positive, "システム");
        storage.save_feedback("ep1", &feedback).await.unwrap();

        // Retrieve and verify
        let feedback_list = storage.find_feedback(None, None).await.unwrap();
        assert_eq!(feedback_list.len(), 1);
        assert_eq!(feedback_list[0].content, "良いスタート");
    }

    #[tokio::test]
    async fn test_save_and_get_feedback() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();
        let episode_id = "ep-feedback";
        let feedback = Feedback::new("良いスタート".to_string(), FeedbackType::Positive, "システム".to_string(), None);
        storage.save_feedback(episode_id, &feedback).await.unwrap();

        let retrieved_feedback = storage.get_feedback(episode_id).await.unwrap();
        assert_eq!(retrieved_feedback.len(), 1);
        assert_eq!(retrieved_feedback[0], feedback);
    }
}
