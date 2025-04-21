use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("State machine error: {0}")]
    StateMachine(#[from] rustate::Error),

    #[error("状態機械が見つかりません: {0}")]
    MachineNotFound(String),

    #[error("イベントが無効です: {0}")]
    InvalidEvent(String),

    #[error("シリアライズエラー: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("内部エラー: {0}")]
    Internal(String),

    #[error("認証エラー: {0}")]
    Authentication(String),

    #[error("認可エラー: {0}")]
    Authorization(String),

    #[error("通信エラー: {0}")]
    Communication(String),

    #[error("コード生成エラー: {0}")]
    CodeGeneration(String),
}

impl From<GrpcError> for tonic::Status {
    fn from(err: GrpcError) -> Self {
        use tonic::Code;
        match err {
            GrpcError::StateMachine(e) => {
                tonic::Status::new(Code::InvalidArgument, format!("State machine error: {}", e))
            }
            GrpcError::MachineNotFound(id) => {
                tonic::Status::new(Code::NotFound, format!("Machine not found: {}", id))
            }
            GrpcError::InvalidEvent(e) => {
                tonic::Status::new(Code::InvalidArgument, format!("Invalid event: {}", e))
            }
            GrpcError::Serialization(e) => {
                tonic::Status::new(Code::Internal, format!("Serialization error: {}", e))
            }
            GrpcError::Internal(e) => tonic::Status::new(Code::Internal, format!("Internal error: {}", e)),
            GrpcError::Authentication(e) => {
                tonic::Status::new(Code::Unauthenticated, format!("Authentication error: {}", e))
            }
            GrpcError::Authorization(e) => {
                tonic::Status::new(Code::PermissionDenied, format!("Authorization error: {}", e))
            }
            GrpcError::Communication(e) => {
                tonic::Status::new(Code::Unavailable, format!("Communication error: {}", e))
            }
            GrpcError::CodeGeneration(e) => {
                tonic::Status::new(Code::Internal, format!("Code generation error: {}", e))
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, GrpcError>; 