use crate::actor::Actor;
use crate::actor_ref::ActorRef;
use crate::spawn::{spawn_actor, DEFAULT_BUFFER_SIZE}; // 既存のspawnロジックと定数をインポート

/// アクターシステムの基本的な実装。
/// 主にトップレベルアクターの生成と管理のエントリーポイントとして機能します。
#[derive(Debug, Clone)] // システムを複数箇所から参照できるように Clone を派生
pub struct ActorSystem {
    name: String,
    // 将来的にシステム全体の設定や、管理対象のアクターへの参照などを持つ可能性あり
}

impl ActorSystem {
    /// 指定された名前で新しいアクターシステムを作成します。
    pub fn new(name: &str) -> Self {
        println!("Initializing ActorSystem '{}'...", name);
        ActorSystem {
            name: name.to_string(),
        }
    }

    /// システムの名前を取得します。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// このシステム内に新しいトップレベルアクターを生成します。
    ///
    /// アクターは独立した非同期タスクで実行されます。
    ///
    /// # Arguments
    /// * `actor` - 生成する `Actor` トレイトを実装したインスタンス。
    /// * `buffer` - アクターのイベントキュー（メールボックス）のサイズ。
    ///
    /// # Returns
    /// 生成されたアクターへの参照 (`ActorRef`)。
    pub fn spawn<A: Actor>(&self, actor: A, buffer: usize) -> ActorRef<A>
    where
        A::State: PartialEq, // spawn_actor が要求するため、この境界は必要
    {
        println!(
            "ActorSystem '{}' spawning actor with buffer size {}...",
            self.name, buffer
        );
        // 既存のspawnロジックを呼び出す
        spawn_actor(actor, buffer)
    }

    /// デフォルトのバッファサイズで新しいトップレベルアクターを生成します。
    ///
    /// # Arguments
    /// * `actor` - 生成する `Actor` トレイトを実装したインスタンス。
    ///
    /// # Returns
    /// 生成されたアクターへの参照 (`ActorRef`)。
    pub fn spawn_default<A: Actor>(&self, actor: A) -> ActorRef<A>
    where
        A::State: PartialEq, // spawn_actor が要求するため、この境界は必要
    {
        println!(
            "ActorSystem '{}' spawning actor with default buffer size...",
            self.name
        );
        self.spawn(actor, DEFAULT_BUFFER_SIZE)
    }

    // 将来的に追加される可能性のあるメソッド:
    // - システム全体のシャットダウン
    // - アクターの検索（限定的なスコープで）
    // - システムレベルの監視設定
}

// ActorSystem 自体がスレッドセーフであることを示すマーカー (内容物に依存)
// 現在の実装では name: String のみなので Send + Sync は自明。
// 将来的に内部状態を持つ場合は注意が必要。
