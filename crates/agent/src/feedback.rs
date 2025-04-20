use crate::{decision::Decision, error::Result};
use async_trait::async_trait;
use rustate::{EventTrait, StateTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};

/// エージェントのフィードバックを表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback<E>
where
    E: EventTrait,
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

impl Feedback<E>
where
    E: EventTrait,
{
    /// 新しいフィードバックを作成します
    pub fn new(score: f64, reason: impl Into<String>) -> Self {
        if !(0.0..=1.0).contains(&score) {
            eprintln!("警告: フィードバックスコアは通常0.0から1.0の範囲です。与えられた値: {}", score);
        }

        Self {
            id: generate_id(),
            score,
            reason: reason.into(),
            metadata: HashMap::new(),
            timestamp: current_timestamp(),
            event: None,
            content: String::new(),
            source: String::new(),
            feedback_type: FeedbackType::Neutral,
        }
    }

    /// フィードバックにメタデータを追加します
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// フィードバックに複数のメタデータを一度に追加します
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    /// このフィードバックが肯定的かどうかを返します（スコアが0.5より大きい場合）
    pub fn is_positive(&self) -> bool {
        self.score > 0.5
    }

    /// このフィードバックが否定的かどうかを返します（スコアが0.5未満の場合）
    pub fn is_negative(&self) -> bool {
        self.score < 0.5
    }

    /// このフィードバックが中立的かどうかを返します（スコアがちょうど0.5の場合）
    pub fn is_neutral(&self) -> bool {
        (self.score - 0.5).abs() < f64::EPSILON
    }
}

/// フィードバックを提供するコンポーネントのトレイト
#[async_trait]
pub trait FeedbackProvider<E>
where
    E: EventTrait,
{
    /// 決定に対するフィードバックを提供します
    async fn provide_feedback(&self, decision: &Decision<E>) -> Result<Feedback<E>>;
}

/// 現在のUNIXタイムスタンプを返します
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// フィードバック用の一意な識別子を生成します
fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = current_timestamp();
    format!("fb-{}-{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_creation() {
        let feedback = Feedback::new(0.8, "優れた決定でした");

        assert_eq!(feedback.score, 0.8);
        assert_eq!(feedback.reason, "優れた決定でした");
        assert!(feedback.metadata.is_empty());
        assert!(feedback.is_positive());
        assert!(!feedback.is_negative());
        assert!(!feedback.is_neutral());
    }

    #[test]
    fn test_feedback_with_metadata() {
        let feedback = Feedback::new(0.3, "改善の余地があります")
            .with_metadata("reviewer", "system")
            .with_metadata("category", "efficiency");

        assert!(feedback.is_negative());
        assert_eq!(feedback.metadata.get("reviewer"), Some(&"system".to_string()));
        assert_eq!(feedback.metadata.get("category"), Some(&"efficiency".to_string()));
    }

    #[test]
    fn test_neutral_feedback() {
        let feedback = Feedback::new(0.5, "中立的な決定");
        assert!(feedback.is_neutral());
        assert!(!feedback.is_positive());
        assert!(!feedback.is_negative());
    }
} 