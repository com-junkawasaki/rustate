use crate::actor::ActorError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// アクターの具体的な状態遷移ロジック、アクション、ガードなどをカプセル化するトレイト。
///
/// 通常、`create_machine` マクロ（フェーズ2で実装予定）がこのトレイトを実装する
/// ステートマシン構造体を自動生成します。
/// 手動で実装することも可能ですが、マクロの使用が推奨されます。
#[async_trait]
pub trait ActorLogic: Send + Sync + 'static {
    /// アクターのコンテキスト（拡張状態）の型。ステートマシンが保持するデータ。
    type Context: Send + Sync + Clone + Debug + Serialize + for<'de> Deserialize<'de>;
    /// アクターが処理するイベントの型。`Actor` トレイトの `Event` と一致する必要があります。
    type Event: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    /// アクターの状態を表す型（通常は enum）。
    type State: Send + Sync + Clone + Debug + PartialEq + Eq + Serialize + for<'de> Deserialize<'de>;

    /// ステートマシンの初期状態と初期コンテキストを返します。
    fn initial(&self) -> (Self::State, Self::Context);

    /// 特定の状態とコンテキストにおいて、指定されたイベントに対する遷移を実行します。
    ///
    /// # Arguments
    /// * `state` - 現在の状態。
    /// * `context` - 現在のコンテキスト。
    /// * `event` - 受信したイベント。
    ///
    /// # Returns
    /// 遷移後の新しい状態とコンテキスト、またはエラー。
    /// 状態やコンテキストが変化しない場合（例: ガードが偽、イベントが未処理）、
    /// 現在の状態とコンテキストをそのまま返すか、特定の `NoTransition` エラーを返します。
    async fn transition(
        &self,
        state: Self::State,
        context: Self::Context,
        event: Self::Event,
    ) -> Result<(Self::State, Self::Context), ActorError>;

    // 将来的に追加される可能性のあるメソッド:
    // - 状態 एंट्री/エグジットアクションの実行
    // - 遷移アクションの実行
    // - ガード条件の評価
}

// ActorLogic を実装した具体的なステートマシンの例 (ダミー)
/*
struct MyMachineLogic;

#[async_trait]
impl ActorLogic for MyMachineLogic {
    type Context = i32;
    type Event = String;
    type State = MyState; // enum MyState { Idle, Processing }

    fn initial(&self) -> (Self::State, Self::Context) {
        (MyState::Idle, 0)
    }

    async fn transition(&self, state: Self::State, context: Self::Context, event: Self::Event)
        -> Result<(Self::State, Self::Context), ActorError>
    {
        // ... 実際の遷移ロジック ...
        Ok((state, context))
    }
}
*/
