use thiserror::Error;

/// RuState統合クレートのエラー型
#[derive(Error, Debug)]
pub enum Error {
    /// ステートマシンエラー
    #[error("ステートマシンエラー: {0}")]
    StateError(#[from] rustate::Error),
    
    /// JSONシリアライズ/デシリアライズエラー
    #[error("JSONエラー: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// ロック取得エラー
    #[error("ロック取得に失敗しました")]
    LockError,
    
    /// 一般的なエラー
    #[error("{0}")]
    Other(String),
}

/// 統合クレートの結果型
pub type Result<T> = std::result::Result<T, Error>;

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Other(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::Other(err.to_string())
    }
}

/// ロックエラーを変換するためのユーティリティトレイト
pub trait LockResultExt<T> {
    /// LockResultをResult<T, Error>に変換
    fn lock_err(self) -> Result<T>;
}

impl<T, E> LockResultExt<T> for std::result::Result<T, E>
where
    E: std::fmt::Debug,
{
    fn lock_err(self) -> Result<T> {
        self.map_err(|_| {
            Error::LockError
        })
    }
} 