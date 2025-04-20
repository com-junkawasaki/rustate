use thiserror::Error;

/// エージェント関連のエラーを表す型
#[derive(Error, Debug)]
pub enum AgentError {
    /// 状態機械関連のエラー
    #[error("状態機械エラー: {0}")]
    MachineError(#[from] rustate::Error),

    /// 決定作成時のエラー
    #[error("決定エラー: {0}")]
    DecisionError(String),

    /// LLMとの通信エラー
    #[error("LLM通信エラー: {0}")]
    LlmError(String),

    /// 観測データの処理エラー
    #[error("観測データエラー: {0}")]
    ObservationError(String),

    /// フィードバック処理エラー
    #[error("フィードバックエラー: {0}")]
    FeedbackError(String),

    /// 永続化/ストレージエラー
    #[error("ストレージエラー: {0}")]
    StorageError(String),

    /// シリアライズ/デシリアライズエラー
    #[error("シリアライズエラー: {0}")]
    SerializationError(String),

    /// ポリシーエラー
    #[error("ポリシーエラー: {0}")]
    PolicyError(String),

    /// エピソードエラー
    #[error("エピソードエラー: {0}")]
    EpisodeError(String),

    /// その他のエラー
    #[error("その他のエラー: {0}")]
    Other(String),
}

/// 結果型エイリアス
pub type Result<T> = std::result::Result<T, AgentError>;

impl From<serde_json::Error> for AgentError {
    fn from(error: serde_json::Error) -> Self {
        AgentError::SerializationError(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentError::DecisionError("テストエラー".to_string());
        assert_eq!(err.to_string(), "決定エラー: テストエラー");
    }
} 