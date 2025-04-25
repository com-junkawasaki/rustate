use thiserror::Error;

/// エージェント関連のエラーを表す型
#[derive(Error, Debug)]
pub enum AgentError {
    /// エージェントが初期化されていない
    #[error("Agent is not initialized")]
    NotInitialized,

    /// 状態機械関連のエラー
    #[error("State machine error: {0}")]
    MachineError(#[from] rustate::Error),

    /// 永続化/ストレージエラー
    #[error("Storage error: {0}")]
    StorageError(String),

    /// ポリシーエラー
    #[error("Policy error: {0}")]
    PolicyError(String),

    /// シリアライズ/デシリアライズエラー
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// デシリアライズエラー
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// 統合エラー
    #[error("Integration error: {0}")]
    IntegrationError(String),

    /// エピソードエラー
    #[error("Episode error: {0}")]
    EpisodeError(String),

    /// 操作がサポートされていない
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// 状態機械内部エラー
    #[error("State machine internal error: {0}")]
    StateMachineError(String),

    /// 内部エージェントエラー
    #[error("Internal agent error: {0}")]
    InternalError(String),

    /// その他のエラー
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for operations that can fail
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
    fn test_agent_error_display() {
        assert_eq!(
            AgentError::NotInitialized.to_string(),
            "Agent is not initialized"
        );
        // Add other display checks if needed
    }

    #[test]
    fn test_error_source() {
        // Example for MachineError
        // let original_error = rustate::Error::InvalidState("some_state".to_string());
        // let agent_error = AgentError::MachineError(original_error);
        // assert!(agent_error.source().is_some());
    }
}
