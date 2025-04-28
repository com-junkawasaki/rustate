use thiserror::Error;
use std::fmt;
use rustate::Error as RustateError;

/// Result type alias using the library's AgentError.
pub type Result<T> = std::result::Result<T, AgentError>;

/// Represents errors that can occur within the agent framework.
#[derive(Error, Debug)]
pub enum AgentError {
    /// エージェントが初期化されていない
    #[error("Agent is not initialized")]
    NotInitialized,

    /// 状態機械関連のエラー
    #[error("State machine error: {0}")]
    MachineError(#[from] RustateError),

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

    /// アクティブなエピソードが見つからない
    #[error("No active episode found")]
    NoActiveEpisode,

    /// エピソードが既にアクティブである
    #[error("Episode is already active")]
    EpisodeAlreadyActive,

    /// 無効な設定
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// I/Oエラー
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// 不明なエラー
    #[error("Unknown error")]
    Unknown,
}

/// Errors related to policy decisions.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// 決定に失敗しました
    #[error("Decision failed: {0}")]
    DecisionFailed(String),

    /// 利用可能なイベントがありません
    #[error("No possible events")]
    NoPossibleEvents,

    /// ゴールの状態が無効です
    #[error("Invalid goal state")]
    InvalidGoalState,
}

/// Errors related to data storage.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Mutexがポイズンされました
    #[error("Mutex poisoned: {0}")]
    MutexPoisoned(String),

    /// アイテムが見つかりません
    #[error("Item not found: {0}")]
    NotFound(String),

    /// ストレージバックエンドエラー
    #[error("Storage backend error: {0}")]
    BackendError(String),

    /// I/Oエラー
    #[error("I/O error: {0}")]
    IoError(String),
}

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
