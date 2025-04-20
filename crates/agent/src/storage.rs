use crate::{
    decision::Decision,
    episode::Episode,
    error::Result,
    insight::Insight,
    observation::Observation,
};
use async_trait::async_trait;
use rustate::{Event, State};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

/// エージェントの経験（観測、決定、洞察、エピソード）を保存するためのトレイト
#[async_trait]
pub trait Storage<S, E>: Send + Sync
where
    S: State + DeserializeOwned + Debug + 'static,
    E: Event + DeserializeOwned + Debug + 'static,
{
    /// 観測データを保存します
    async fn save_observation(&self, observation: &Observation<S, E>) -> Result<()>;

    /// 観測データを取得します
    async fn get_observation(&self, id: &str) -> Result<Observation<S, E>>;

    /// 条件に一致する観測データを検索します
    async fn find_observations(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>>;

    /// 決定を保存します
    async fn save_decision(&self, decision: &Decision<E>) -> Result<()>;

    /// 決定を取得します
    async fn get_decision(&self, id: &str) -> Result<Decision<E>>;

    /// 条件に一致する決定を検索します
    async fn find_decisions(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>>;

    /// 洞察を保存します
    async fn save_insight(&self, insight: &Insight) -> Result<()>;

    /// 洞察を取得します
    async fn get_insight(&self, id: &str) -> Result<Insight>;

    /// 条件に一致する洞察を検索します
    async fn find_insights(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>>;

    /// エピソードを保存します
    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<()>;

    /// エピソードを取得します
    async fn get_episode(&self, id: &str) -> Result<Episode<S, E>>;

    /// 条件に一致するエピソードを検索します
    async fn find_episodes(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>>;
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

/// インメモリストレージの実装
pub struct MemoryStorage<S, E>
where
    S: State + Clone,
    E: Event + Clone,
{
    observations: Arc<Mutex<Vec<Observation<S, E>>>>,
    decisions: Arc<Mutex<Vec<Decision<E>>>>,
    insights: Arc<Mutex<Vec<Insight>>>,
    episodes: Arc<Mutex<Vec<Episode<S, E>>>>,
}

impl<S, E> MemoryStorage<S, E>
where
    S: State + Clone,
    E: Event + Clone,
{
    /// 新しいインメモリストレージを作成します
    pub fn new() -> Self {
        Self {
            observations: Arc::new(Mutex::new(Vec::new())),
            decisions: Arc::new(Mutex::new(Vec::new())),
            insights: Arc::new(Mutex::new(Vec::new())),
            episodes: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl<S, E> Storage<S, E> for MemoryStorage<S, E>
where
    S: State + DeserializeOwned + Debug + Clone + Send + Sync + 'static,
    E: Event + DeserializeOwned + Debug + Clone + Send + Sync + 'static,
{
    async fn save_observation(&self, observation: &Observation<S, E>) -> Result<()> {
        let mut observations = self.observations.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        observations.push(observation.clone());
        Ok(())
    }

    async fn get_observation(&self, id: &str) -> Result<Observation<S, E>> {
        let observations = self.observations.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        observations
            .iter()
            .find(|obs| obs.id == id)
            .cloned()
            .ok_or_else(|| crate::error::AgentError::StorageError(format!("観測 ID {} が見つかりません", id)))
    }

    async fn find_observations(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Observation<S, E>>> {
        let observations = self.observations.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        
        let mut results = observations.clone();

        if let Some(q) = query {
            // タイムスタンプでフィルタリング
            if let Some(from) = q.from_timestamp {
                results.retain(|obs| obs.timestamp >= from);
            }
            if let Some(to) = q.to_timestamp {
                results.retain(|obs| obs.timestamp <= to);
            }

            // フィールドでフィルタリング
            for (field, value) in &q.filters {
                match field.as_str() {
                    "id" => results.retain(|obs| obs.id.contains(value)),
                    // 他のフィールドのフィルタリングはここに追加
                    _ => {}
                }
            }

            // ソート（タイムスタンプベース）
            results.sort_by(|a, b| {
                if q.sort_descending {
                    b.timestamp.cmp(&a.timestamp)
                } else {
                    a.timestamp.cmp(&b.timestamp)
                }
            });
        }

        // 結果を制限
        if let Some(l) = limit {
            results.truncate(l);
        }

        Ok(results)
    }

    async fn save_decision(&self, decision: &Decision<E>) -> Result<()> {
        let mut decisions = self.decisions.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        decisions.push(decision.clone());
        Ok(())
    }

    async fn get_decision(&self, id: &str) -> Result<Decision<E>> {
        let decisions = self.decisions.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        decisions
            .iter()
            .find(|dec| dec.id == id)
            .cloned()
            .ok_or_else(|| crate::error::AgentError::StorageError(format!("決定 ID {} が見つかりません", id)))
    }

    async fn find_decisions(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Decision<E>>> {
        let decisions = self.decisions.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        
        let mut results = decisions.clone();

        if let Some(q) = query {
            // タイムスタンプでフィルタリング
            if let Some(from) = q.from_timestamp {
                results.retain(|dec| dec.timestamp >= from);
            }
            if let Some(to) = q.to_timestamp {
                results.retain(|dec| dec.timestamp <= to);
            }

            // ソート
            results.sort_by(|a, b| {
                if q.sort_descending {
                    b.timestamp.cmp(&a.timestamp)
                } else {
                    a.timestamp.cmp(&b.timestamp)
                }
            });
        }

        // 結果を制限
        if let Some(l) = limit {
            results.truncate(l);
        }

        Ok(results)
    }

    async fn save_insight(&self, insight: &Insight) -> Result<()> {
        let mut insights = self.insights.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        insights.push(insight.clone());
        Ok(())
    }

    async fn get_insight(&self, id: &str) -> Result<Insight> {
        let insights = self.insights.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        insights
            .iter()
            .find(|ins| ins.id == id)
            .cloned()
            .ok_or_else(|| crate::error::AgentError::StorageError(format!("洞察 ID {} が見つかりません", id)))
    }

    async fn find_insights(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Insight>> {
        let insights = self.insights.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        
        let mut results = insights.clone();

        if let Some(q) = query {
            // タイムスタンプでフィルタリング
            if let Some(from) = q.from_timestamp {
                results.retain(|ins| ins.timestamp >= from);
            }
            if let Some(to) = q.to_timestamp {
                results.retain(|ins| ins.timestamp <= to);
            }

            // ソート
            results.sort_by(|a, b| {
                if q.sort_descending {
                    b.timestamp.cmp(&a.timestamp)
                } else {
                    a.timestamp.cmp(&b.timestamp)
                }
            });
        }

        // 結果を制限
        if let Some(l) = limit {
            results.truncate(l);
        }

        Ok(results)
    }

    async fn save_episode(&self, episode: &Episode<S, E>) -> Result<()> {
        let mut episodes = self.episodes.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        episodes.push(episode.clone());
        Ok(())
    }

    async fn get_episode(&self, id: &str) -> Result<Episode<S, E>> {
        let episodes = self.episodes.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        episodes
            .iter()
            .find(|ep| ep.id == id)
            .cloned()
            .ok_or_else(|| crate::error::AgentError::StorageError(format!("エピソード ID {} が見つかりません", id)))
    }

    async fn find_episodes(
        &self,
        query: Option<&StorageQuery>,
        limit: Option<usize>,
    ) -> Result<Vec<Episode<S, E>>> {
        let episodes = self.episodes.lock().map_err(|e| {
            crate::error::AgentError::StorageError(format!("ロック取得エラー: {}", e))
        })?;
        
        let mut results = episodes.clone();

        if let Some(q) = query {
            // タイムスタンプでフィルタリング
            if let Some(from) = q.from_timestamp {
                results.retain(|ep| ep.start_time >= from);
            }
            if let Some(to) = q.to_timestamp {
                results.retain(|ep| {
                    ep.end_time.map_or(true, |end_time| end_time <= to)
                });
            }

            // ソート
            results.sort_by(|a, b| {
                if q.sort_descending {
                    b.start_time.cmp(&a.start_time)
                } else {
                    a.start_time.cmp(&b.start_time)
                }
            });
        }

        // 結果を制限
        if let Some(l) = limit {
            results.truncate(l);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_memory_storage_decisions() {
        let storage = MemoryStorage::<TestState, TestEvent>::new();

        let decision = Decision::new(TestEvent::Start, "テスト決定");

        // 決定を保存
        storage.save_decision(&decision).await.unwrap();

        // 決定を取得
        let retrieved = storage.get_decision(&decision.id).await.unwrap();
        assert_eq!(retrieved.id, decision.id);
    }
} 