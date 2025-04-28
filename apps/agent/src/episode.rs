use crate::goal::Goal;
use crate::{decision::Decision, feedback::Feedback, insight::Insight, observation::Observation};
use rustate::{EventTrait, StateTrait};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// エピソードは、初期状態から目標状態までの一連の観測、決定、フィードバック、洞察を含む
/// 完全なシーケンスを表します。
///
/// 強化学習におけるエピソードと同様に、エージェントの学習と評価の単位となります。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    bound = "S: Serialize + for<'deserialize> Deserialize<'deserialize>, E: Serialize + for<'deserialize> Deserialize<'deserialize>"
)]
pub struct Episode<S, E>
where
    S: StateTrait + Send + Sync + Clone + Debug + 'static + DeserializeOwned,
    E: EventTrait + Send + Sync + Debug + Clone + 'static + DeserializeOwned,
{
    /// エピソードの一意な識別子
    pub id: Uuid,

    /// エピソードの名前または説明
    pub name: String,

    /// エピソードの開始時間（UNIXタイムスタンプ）
    pub start_time: SystemTime,

    /// エピソードの終了時間（UNIXタイムスタンプ）、完了していない場合はNone
    pub end_time: Option<SystemTime>,

    /// エピソードの初期状態
    pub initial_state: S,

    /// エピソードの目標状態（ある場合）
    pub goal: Goal<S>,

    /// エピソード中に記録された観測データ
    pub observations: Vec<Observation<S, E>>,

    /// エピソード中に行われた決定
    pub decisions: Vec<Decision<E>>,

    /// エピソード中に生成された洞察
    pub insights: Vec<Insight>,

    /// このエピソードに関連する追加のメタデータ
    pub metadata: serde_json::Value,

    /// エピソードの成功または失敗を示す
    pub is_successful: bool,

    /// エピソードの総合評価（0.0〜1.0）
    pub overall_score: f64,

    /// 受け取ったフィードバック
    pub feedback: Option<Feedback<E>>,
}

impl<S, E> Episode<S, E>
where
    S: StateTrait + Send + Sync + Clone + Debug + 'static + DeserializeOwned,
    E: EventTrait + Send + Sync + Debug + Clone + 'static + DeserializeOwned,
{
    /// 新しいエピソードを作成します
    pub fn new(name: impl Into<String>, initial_state: S, goal: Goal<S>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            start_time: SystemTime::now(),
            end_time: None,
            initial_state,
            goal,
            observations: Vec::new(),
            decisions: Vec::new(),
            insights: Vec::new(),
            metadata: serde_json::Value::Null,
            is_successful: false,
            overall_score: 0.0,
            feedback: None,
        }
    }

    /// エピソードに観測データを追加します
    pub fn add_observation(&mut self, observation: Observation<S, E>) {
        self.observations.push(observation);
    }

    /// エピソードに決定を追加します
    pub fn add_decision(&mut self, decision: Decision<E>) {
        self.decisions.push(decision);
    }

    /// エピソードに洞察を追加します
    pub fn add_insight(&mut self, insight: Insight) -> &mut Self {
        self.insights.push(insight);
        self
    }

    /// エピソードにフィードバックを追加します
    pub fn add_feedback(&mut self, feedback: Feedback<E>) -> &mut Self {
        self.feedback = Some(feedback);
        self
    }

    /// エピソードを完了としてマークし、成功または失敗を設定します
    pub fn complete(&mut self, is_successful: bool) {
        self.end_time = Some(SystemTime::now());
        self.is_successful = is_successful;
    }

    /// エピソードに総合評価を設定します
    pub fn set_overall_score(&mut self, score: f64) {
        if !(0.0..=1.0).contains(&score) {
            eprintln!(
                "警告: エピソードスコアは通常0.0から1.0の範囲です。与えられた値: {}",
                score
            );
        }
        self.overall_score = score;
    }

    /// エピソードにメタデータを追加します
    pub fn add_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> Result<&mut Self, serde_json::Error> {
        // Convert existing metadata to object if it's not already
        let metadata_obj = match &self.metadata {
            serde_json::Value::Object(obj) => obj.clone(),
            _ => serde_json::Map::new(),
        };

        // Convert the value to a JSON value
        let json_value = serde_json::to_value(value)?;

        // Create a new map and insert the value
        let mut new_metadata = metadata_obj;
        new_metadata.insert(key.into(), json_value);

        // Update the metadata field
        self.metadata = serde_json::Value::Object(new_metadata);

        Ok(self)
    }

    /// エピソードの期間を秒単位で返します
    pub fn duration_seconds(&self) -> Option<u64> {
        match self.end_time {
            Some(end) => end
                .duration_since(self.start_time)
                .ok()
                .map(|d| d.as_secs()),
            None => SystemTime::now()
                .duration_since(self.start_time)
                .ok()
                .map(|d| d.as_secs()),
        }
    }

    /// エピソードが完了しているかどうかを返します
    pub fn is_completed(&self) -> bool {
        self.end_time.is_some()
    }

    /// エピソードのすべての決定に関連するフィードバックを収集します
    pub fn collect_feedback(&self) -> Vec<&Feedback<E>> {
        let mut all_feedback = Vec::new();

        // エピソード全体のフィードバックがあれば追加
        if let Some(feedback) = &self.feedback {
            all_feedback.push(feedback);
        }

        // 決定に関連するフィードバックは現在の実装では収集できません
        // Decision構造体にfeedbackフィールドがないため

        all_feedback
    }

    /// エピソードの平均フィードバックスコアを計算します
    pub fn average_feedback_score(&self) -> Option<f64> {
        let all_feedback = self.collect_feedback();

        if all_feedback.is_empty() {
            return None;
        }

        let total_score: f64 = all_feedback
            .iter()
            .map(|f| match f.feedback_type {
                crate::feedback::FeedbackType::Positive => 1.0,
                crate::feedback::FeedbackType::Neutral => 0.5,
                crate::feedback::FeedbackType::Negative => 0.0,
            })
            .sum();

        Some(total_score / all_feedback.len() as f64)
    }
}

/// Helper function to get the current timestamp.
#[allow(dead_code)]
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Helper function to generate a unique ID.
#[allow(dead_code)]
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp();
    format!("ep-{}-{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_episode_creation() {
        let initial = TestState::Initial;
        let goal_state = TestState::Final;
        let goal: Goal<TestState> = Goal::new(goal_state.clone());
        let episode: Episode<TestState, TestEvent> =
            Episode::new("Test Episode", initial.clone(), goal.clone());

        assert_eq!(episode.name, "Test Episode");
        assert_eq!(episode.initial_state, initial);
        assert_eq!(episode.goal.target_state, goal_state);
        assert!(episode.end_time.is_none());
        assert!(!episode.is_successful);
        assert!(episode.observations.is_empty());
        assert!(episode.decisions.is_empty());
        assert!(episode.insights.is_empty());
        assert_eq!(episode.metadata, serde_json::Value::Null);
    }

    #[test]
    fn test_episode_with_observations_and_decisions() {
        let initial = TestState::Initial;
        let goal: Goal<TestState> = Goal::new(TestState::Final);
        let mut episode: Episode<TestState, TestEvent> =
            Episode::new("Test Episode 2", initial.clone(), goal.clone());

        let obs = Observation::new(initial, TestEvent::Start, TestState::Processing);

        let insight = Insight::new("Test Insight", "This is a test insight", 0.9);

        let decision = Decision::simple(TestEvent::Start, 0.9);

        episode.add_observation(obs);
        episode.add_decision(decision.clone());
        episode.add_insight(insight);

        assert_eq!(episode.observations.len(), 1);
        assert_eq!(episode.decisions.len(), 1);
        assert_eq!(episode.insights.len(), 1);
    }

    #[test]
    fn test_episode_completion() {
        let initial = TestState::Initial;
        let goal: Goal<TestState> = Goal::new(TestState::Final);
        let mut episode: Episode<TestState, TestEvent> =
            Episode::new("Test Episode 3", initial, goal.clone());

        assert!(!episode.is_completed());
        assert!(episode.end_time.is_none());

        episode.complete(true);

        assert_eq!(episode.is_completed(), true);
        assert!(episode.end_time.is_some());
        assert_eq!(episode.is_successful, true);
    }

    #[test]
    fn test_episode_feedback() {
        let initial = TestState::Initial;
        let goal: Goal<TestState> = Goal::new(TestState::Final);
        let mut episode: Episode<TestState, TestEvent> =
            Episode::new("Test Episode 4", initial, goal.clone());

        let feedback: Feedback<TestEvent> =
            Feedback::new("良い選択", crate::feedback::FeedbackType::Positive, "user");
        episode.add_feedback(feedback);

        assert!(episode.feedback.is_some());
        assert_eq!(episode.feedback.as_ref().unwrap().content, "良い選択");
        assert_eq!(
            episode.feedback.as_ref().unwrap().feedback_type,
            crate::feedback::FeedbackType::Positive
        );
    }

    #[test]
    fn test_add_metadata() {
        let initial = TestState::Initial;
        let goal: Goal<TestState> = Goal::new(TestState::Final);
        let mut episode: Episode<TestState, TestEvent> =
            Episode::new("Test Episode 5", initial, goal.clone());

        episode.add_metadata("priority", "high").unwrap();
        episode
            .add_metadata("tags", vec!["important", "urgent"])
            .unwrap();

        if let serde_json::Value::Object(map) = &episode.metadata {
            assert_eq!(map.get("priority").unwrap(), "high");
            assert!(map.get("tags").is_some());
        } else {
            panic!("メタデータはオブジェクトであるべき");
        }
    }
}
