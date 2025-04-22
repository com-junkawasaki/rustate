use thiserror::Error;

/// エージェント関連のエラーを表す型
#[derive(Error, Debug)]
pub enum AgentError {
    /// 状態機械関連のエラー
    #[error("Machine error: {0}")]
    MachineError(#[from] rustate::Error),

    /// 決定作成時のエラー
    #[error("Decision error: {0}")]
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
    #[error("Storage error: {0}")]
    StorageError(String),

    /// シリアライズ/デシリアライズエラー
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// ポリシーエラー
    #[error("Policy error: {0}")]
    PolicyError(String),

    /// エピソードエラー
    #[error("エピソードエラー: {0}")]
    EpisodeError(String),

    /// アクティブなエピソードがない
    #[error("No active episode")]
    NoActiveEpisode,

    /// 目標状態が定義されていない
    #[error("No goal defined")]
    NoGoalDefined,

    /// その他のエラー
    #[error("Other error: {0}")]
    Other(String),

    /// 統合エラー
    #[error("Integration error: {0}")]
    IntegrationError(String),

    /// 無効な状態エラー
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// 無効な遷移エラー
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),

    /// I/Oエラー
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 結果型エイリアス
pub type Result<T> = std::result::Result<T, AgentError>;

impl From<serde_json::Error> for AgentError {
    fn from(error: serde_json::Error) -> Self {
        AgentError::SerializationError(error.to_string())
    }
}

impl From<rustate::integration::Error> for AgentError {
    fn from(err: rustate::integration::Error) -> Self {
        AgentError::IntegrationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentError::Other("テストエラー".to_string());
        assert_eq!(err.to_string(), "Other error: テストエラー");
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "ファイルが見つかりません");
        let agent_err: AgentError = io_err.into();
        assert!(matches!(agent_err, AgentError::IoError(_)));
    }
}
