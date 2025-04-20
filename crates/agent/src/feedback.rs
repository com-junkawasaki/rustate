use crate::{decision::Decision, error::AgentError};
use async_trait::async_trait;
use rustate::EventTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::Value;

/// フィードバックの種類
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FeedbackType {
    /// 肯定的なフィードバック
    Positive,
    /// 否定的なフィードバック
    Negative,
    /// 中立的なフィードバック
    Neutral,
}

/// エージェントのフィードバックを表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback<E>
where
    E: EventTrait + Clone,
{
    /// フィードバックの一意な識別子
    pub id: String,
    
    /// フィードバックが作成された時間（UNIXタイムスタンプ）
    pub timestamp: u64,
    
    /// フィードバックに関連するイベント
    pub event: Option<E>,
    
    /// フィードバックの内容
    pub content: String,
    
    /// フィードバックのソース（ユーザー、システム、など）
    pub source: String,
    
    /// フィードバックの種類（肯定的、否定的、中立）
    pub feedback_type: FeedbackType,
    
    /// この決定に関連する追加のメタデータ
    pub metadata: HashMap<String, String>,
}

impl<E> Feedback<E>
where
    E: EventTrait + Clone,
{
    /// 新しいフィードバックを作成します
    pub fn new(content: impl Into<String>, feedback_type: FeedbackType, source: impl Into<String>) -> Self {
        Self {
            id: format!("feedback-{}", uuid::Uuid::new_v4()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("時間が取得できませんでした")
                .as_secs(),
            event: None,
            content: content.into(),
            source: source.into(),
            feedback_type,
            metadata: HashMap::new(),
        }
    }
    
    /// メタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// イベントを関連付けます
    pub fn with_event(mut self, event: E) -> Self {
        self.event = Some(event);
        self
    }
    
    /// このフィードバックが肯定的かどうかを返します
    pub fn is_positive(&self) -> bool {
        self.feedback_type == FeedbackType::Positive
    }

    /// このフィードバックが否定的かどうかを返します
    pub fn is_negative(&self) -> bool {
        self.feedback_type == FeedbackType::Negative
    }

    /// このフィードバックが中立的かどうかを返します
    pub fn is_neutral(&self) -> bool {
        self.feedback_type == FeedbackType::Neutral
    }
}

/// フィードバックを提供するコンポーネントのトレイト
#[async_trait]
pub trait FeedbackProvider<E>
where
    E: EventTrait + Clone,
{
    /// 決定に対するフィードバックを提供します
    async fn provide_feedback(&self, decision: &Decision<E>) -> std::result::Result<Feedback<E>, AgentError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustate::EventTrait;
    
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestEvent {
        Action1,
        Action2,
    }
    
    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Action1 => "action1",
                TestEvent::Action2 => "action2",
            }
        }
        
        fn payload(&self) -> Option<&Value> {
            None
        }
    }
    
    #[test]
    fn test_feedback_creation() {
        let feedback = Feedback::<TestEvent>::new("優れた決定でした", FeedbackType::Positive, "user");

        assert_eq!(feedback.content, "優れた決定でした");
        assert_eq!(feedback.source, "user");
        assert!(feedback.metadata.is_empty());
        assert!(feedback.is_positive());
        assert!(!feedback.is_negative());
        assert!(!feedback.is_neutral());
    }

    #[test]
    fn test_feedback_with_metadata() {
        let feedback = Feedback::<TestEvent>::new("改善の余地があります", FeedbackType::Negative, "user")
            .with_metadata("reviewer", "system")
            .with_metadata("category", "efficiency");

        assert!(feedback.is_negative());
        assert_eq!(feedback.metadata.get("reviewer"), Some(&"system".to_string()));
        assert_eq!(feedback.metadata.get("category"), Some(&"efficiency".to_string()));
    }

    #[test]
    fn test_neutral_feedback() {
        let feedback = Feedback::<TestEvent>::new("中立的な決定", FeedbackType::Neutral, "user");
        assert!(feedback.is_neutral());
        assert!(!feedback.is_positive());
        assert!(!feedback.is_negative());
    }
}
