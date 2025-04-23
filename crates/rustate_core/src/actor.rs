use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;

/// アクターが処理中に発生させる可能性のあるエラー。
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorError {
    #[error("メッセージの処理に失敗しました: {0}")]
    ProcessingFailed(String),
    #[error("予期せぬイベント型を受信しました")]
    UnexpectedEvent,
    #[error("アクターが停止しました")]
    Stopped,
    // 他のエラーケースを追加可能
}

/// すべてのアクターが実装する必要があるコアトレイト。
/// アクターは状態を持ち、イベントを受信して状態を更新し、出力を生成します。
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    /// アクターが保持する内部状態の型。
    type State: Send + Sync + Clone + Debug + Serialize + for<'de> Deserialize<'de>;
    /// アクターが受信するイベントの型。
    type Event: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    /// アクターが生成する出力の型（オプション）。
    type Output: Send + Sync + Clone + Debug + Serialize + for<'de> Deserialize<'de>;

    /// アクターの初期状態を返します。
    fn initial_state(&self) -> Self::State;

    /// イベントを受信し、それに基づいてアクターの状態を更新または他のアクションを実行します。
    /// このメソッドはアクターのメインロジックループの一部です。
    ///
    /// # Arguments
    /// * `state` - 現在のアクターの状態。
    /// * `event` - 受信したイベント。
    ///
    /// # Returns
    /// 新しい状態、またはエラー。状態が変化しない場合は現在の状態を返します。
    async fn receive(
        &self,
        state: Self::State,
        event: Self::Event,
    ) -> Result<Self::State, ActorError>;

    // 将来的に追加される可能性のあるメソッド:
    // - アクターのライフサイクルメソッド (pre_start, post_stop など)
    // - 状態遷移後の副作用 (compute_output など)
}
