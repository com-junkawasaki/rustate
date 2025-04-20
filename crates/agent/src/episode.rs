use crate::{
    decision::Decision,
    feedback::Feedback,
    insight::Insight,
    observation::Observation,
};
use rustate::{Event, State};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// エピソードは、初期状態から目標状態までの一連の観測、決定、フィードバック、洞察を含む
/// 完全なシーケンスを表します。
/// 
/// 強化学習におけるエピソードと同様に、エージェントの学習と評価の単位となります。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode<S, E>
where
    S: State,
    E: Event,
{
    /// エピソードの一意な識別子
    pub id: String,

    /// エピソードの名前または説明
    pub name: String,

    /// エピソードの開始時間（UNIXタイムスタンプ）
    pub start_time: u64,

    /// エピソードの終了時間（UNIXタイムスタンプ）、完了していない場合はNone
    pub end_time: Option<u64>,

    /// エピソードの初期状態
    pub initial_state: S,

    /// エピソードの目標状態（ある場合）
    pub goal_state: Option<S>,

    /// エピソード中に記録された観測データ
    pub observations: Vec<Observation<S, E>>,

    /// エピソード中に行われた決定
    pub decisions: Vec<Decision<E>>,

    /// エピソード中に生成された洞察
    pub insights: Vec<Insight>,

    /// このエピソードに関連する追加のメタデータ
    pub metadata: HashMap<String, String>,

    /// エピソードの成功または失敗を示す
    pub is_successful: Option<bool>,

    /// エピソードの総合評価（0.0〜1.0）
    pub overall_score: Option<f64>,
}

impl<S, E> Episode<S, E>
where
    S: State,
    E: Event,
{
    /// 新しいエピソードを作成します
    pub fn new(name: impl Into<String>, initial_state: S, goal_state: Option<S>) -> Self {
        Self {
            id: generate_id(),
            name: name.into(),
            start_time: current_timestamp(),
            end_time: None,
            initial_state,
            goal_state,
            observations: Vec::new(),
            decisions: Vec::new(),
            insights: Vec::new(),
            metadata: HashMap::new(),
            is_successful: None,
            overall_score: None,
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
    pub fn add_insight(&mut self, insight: Insight) {
        self.insights.push(insight);
    }

    /// エピソードを完了としてマークし、成功または失敗を設定します
    pub fn complete(&mut self, is_successful: bool) {
        self.end_time = Some(current_timestamp());
        self.is_successful = Some(is_successful);
    }

    /// エピソードに総合評価を設定します
    pub fn set_overall_score(&mut self, score: f64) {
        if !(0.0..=1.0).contains(&score) {
            eprintln!("警告: エピソードスコアは通常0.0から1.0の範囲です。与えられた値: {}", score);
        }
        self.overall_score = Some(score);
    }

    /// エピソードにメタデータを追加します
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// エピソードの期間を秒単位で返します
    pub fn duration_seconds(&self) -> Option<u64> {
        self.end_time.map(|end| end - self.start_time)
    }

    /// エピソードが完了しているかどうかを返します
    pub fn is_completed(&self) -> bool {
        self.end_time.is_some()
    }

    /// エピソードのすべての決定に関連するフィードバックを収集します
    pub fn collect_feedback(&self) -> Vec<&Feedback> {
        self.decisions
            .iter()
            .filter_map(|decision| decision.feedback.as_ref())
            .collect()
    }

    /// エピソードの平均フィードバックスコアを計算します
    pub fn average_feedback_score(&self) -> Option<f64> {
        let feedback = self.collect_feedback();
        if feedback.is_empty() {
            return None;
        }

        let sum: f64 = feedback.iter().map(|f| f.score).sum();
        Some(sum / feedback.len() as f64)
    }
}

/// 現在のUNIXタイムスタンプを返します
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// エピソード用の一意な識別子を生成します
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
    use crate::feedback::Feedback;
    use rustate::{EventTrait, StateTrait};

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

    #[test]
    fn test_episode_creation() {
        let episode = Episode::new(
            "テストエピソード",
            TestState::Initial,
            Some(TestState::Final),
        );

        assert_eq!(episode.name, "テストエピソード");
        assert_eq!(episode.initial_state, TestState::Initial);
        assert_eq!(episode.goal_state, Some(TestState::Final));
        assert!(episode.observations.is_empty());
        assert!(episode.decisions.is_empty());
        assert!(episode.insights.is_empty());
        assert!(!episode.is_completed());
    }

    #[test]
    fn test_episode_with_observations_and_decisions() {
        let mut episode = Episode::new(
            "テストエピソード",
            TestState::Initial,
            Some(TestState::Final),
        );

        let observation = Observation::new(
            TestState::Initial,
            TestEvent::Start,
            TestState::Processing,
        );
        episode.add_observation(observation);

        let decision = Decision::new(TestEvent::Process, "処理を進めるため");
        episode.add_decision(decision);

        assert_eq!(episode.observations.len(), 1);
        assert_eq!(episode.decisions.len(), 1);
    }

    #[test]
    fn test_episode_completion() {
        let mut episode = Episode::new(
            "テストエピソード",
            TestState::Initial,
            Some(TestState::Final),
        );

        episode.complete(true);
        assert!(episode.is_completed());
        assert_eq!(episode.is_successful, Some(true));
        assert!(episode.duration_seconds().is_some());
    }

    #[test]
    fn test_episode_feedback() {
        let mut episode = Episode::new(
            "テストエピソード",
            TestState::Initial,
            Some(TestState::Final),
        );

        let decision1 = Decision::new(TestEvent::Start, "開始するため")
            .with_feedback(Feedback::new(0.8, "良い選択"));

        let decision2 = Decision::new(TestEvent::Process, "処理を進めるため")
            .with_feedback(Feedback::new(0.6, "普通の選択"));

        episode.add_decision(decision1);
        episode.add_decision(decision2);

        let feedback = episode.collect_feedback();
        assert_eq!(feedback.len(), 2);
        
        let avg_score = episode.average_feedback_score().unwrap();
        assert!((avg_score - 0.7).abs() < f64::EPSILON);
    }
} 