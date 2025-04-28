use crate::episode::Episode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::{Debug};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use rustate::{StateTrait, EventTrait};
use crate::observation::Observation;

/// 洞察は、観測データに基づく追加情報や解釈を提供します。
/// 洞察はAIエージェントが状態遷移や観測データから抽出した
/// より高度な理解や知見を表します。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Insight {
    /// 洞察の一意な識別子
    pub id: String,

    /// 洞察の種類（例: "パターン検出", "予測", "因果関係"）
    pub insight_type: String,

    /// 洞察の内容または説明
    pub content: String,

    /// この洞察の信頼度（0.0〜1.0）
    pub confidence: f64,

    /// この洞察に関連する追加のメタデータ
    pub metadata: HashMap<String, String>,

    /// この洞察が作成された時間（UNIXタイムスタンプ）
    pub timestamp: u64,

    /// この洞察に関連する観測データの識別子リスト
    pub related_observation_ids: Vec<String>,
}

impl Insight {
    /// 新しい洞察を作成します
    pub fn new(
        insight_type: impl Into<String>,
        content: impl Into<String>,
        confidence: f64,
    ) -> Self {
        if !(0.0..=1.0).contains(&confidence) {
            eprintln!(
                "警告: 洞察の信頼度は通常0.0から1.0の範囲です。与えられた値: {}",
                confidence
            );
        }

        Self {
            id: generate_id(),
            insight_type: insight_type.into(),
            content: content.into(),
            confidence,
            metadata: HashMap::new(),
            timestamp: current_timestamp(),
            related_observation_ids: Vec::new(),
        }
    }

    /// 洞察にメタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 洞察に複数のメタデータを一度に追加します
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    /// 洞察に関連する観測データを追加します
    pub fn with_related_observation<S, E>(mut self, observation: &Observation<S, E>) -> Self
    where
        S: StateTrait + DeserializeOwned,
        E: EventTrait + DeserializeOwned,
    {
        self.related_observation_ids.push(observation.id.clone());
        self
    }

    /// 洞察に複数の関連観測データを一度に追加します
    pub fn with_related_observations<S, E>(mut self, observations: &[Observation<S, E>]) -> Self
    where
        S: StateTrait + DeserializeOwned,
        E: EventTrait + DeserializeOwned,
    {
        for observation in observations {
            self.related_observation_ids.push(observation.id.clone());
        }
        self
    }

    /// 洞察の信頼性が高いかどうかを返します（信頼度が0.7以上の場合）
    pub fn is_reliable(&self) -> bool {
        self.confidence >= 0.7
    }
}

/// 現在のUNIXタイムスタンプを返します
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// 洞察用の一意な識別子を生成します
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp();
    format!("ins-{}-{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::Observation;
    use rustate::{EventTrait, StateTrait};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::fmt::{self, Display, Formatter};
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    #[test]
    fn test_insight_creation() {
        let insight = Insight::new(
            "パターン検出",
            "ユーザーは通常、朝に最初のリクエストを行います",
            0.85,
        );

        assert_eq!(insight.insight_type, "パターン検出");
        assert_eq!(
            insight.content,
            "ユーザーは通常、朝に最初のリクエストを行います"
        );
        assert_eq!(insight.confidence, 0.85);
        assert!(insight.metadata.is_empty());
        assert!(insight.related_observation_ids.is_empty());
        assert!(insight.is_reliable());
    }

    #[test]
    fn test_insight_with_metadata() {
        let insight = Insight::new("予測", "次のアクションはProcess", 0.6)
            .with_metadata("source", "historical data")
            .with_metadata("model", "heuristic");

        assert!(!insight.is_reliable());
        assert_eq!(
            insight.metadata.get("source"),
            Some(&"historical data".to_string())
        );
        assert_eq!(
            insight.metadata.get("model"),
            Some(&"heuristic".to_string())
        );
    }

    #[test]
    fn test_insight_with_related_observations() {
        let obs1 = Observation::new(TestState::Initial, TestEvent::Start, TestState::Processing);

        let obs2 = Observation::new(
            TestState::Processing,
            TestEvent::Process,
            TestState::Processing,
        );

        let insight = Insight::new("因果関係", "イベントAはいつも状態Bに導く", 0.9)
            .with_related_observation(&obs1)
            .with_related_observation(&obs2);

        assert_eq!(insight.related_observation_ids.len(), 2);
        assert_eq!(insight.related_observation_ids[0], obs1.id);
        assert_eq!(insight.related_observation_ids[1], obs2.id);
    }
}
