use crate::actor::{Actor, ActorError};
use std::fmt::Debug;
use tokio::sync::mpsc;

/// アクターへの参照を表し、アクターとのインタラクション（主にイベント送信）を提供します。
///
/// `ActorRef` はアクターインスタンスへの直接的なポインタではなく、
/// アクターシステムの管理下にあるアクターへのハンドルです。
/// これにより、アクターの場所（ローカル、リモートなど）を抽象化できます。
#[derive(Clone)]
pub struct ActorRef<A: Actor> {
    id: String, // デバッグや識別のためのID (UUIDなど)
    // アクターのメールボックスへの送信チャネル
    sender: mpsc::Sender<A::Event>,
}

impl<A: Actor> Debug for ActorRef<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef")
         .field("id", &self.id)
         // sender は通常デバッグ情報には含めないか、アドレス程度にする
         .field("sender", &"...") 
         .finish()
    }
}

impl<A: Actor> ActorRef<A> {
    /// アクターにイベントを非同期で送信します。
    ///
    /// このメソッドは通常ノンブロッキングであり、イベントをアクターのキューに入れて即座に返ります。
    /// 送信先のキューが一杯の場合、キューに空きができるまで待機します。
    ///
    /// # Arguments
    /// * `event` - 送信するイベント。アクターの `Event` 型と一致する必要があります。
    ///
    /// # Returns
    /// 送信が成功した場合は `Ok(())`、失敗した場合は `Err(ActorError)`。
    /// 失敗の主な理由は、受信側のアクター（対応する Receiver）が既にドロップされている（アクターが停止した）場合です。
    pub async fn send(&self, event: A::Event) -> Result<(), ActorError> {
        self.sender
            .send(event)
            .await
            .map_err(|_send_error| {
                // SendError<T> は T を含みますが、ここではイベント自体は不要
                // エラーの主な原因は受信側の消失なので Stopped とする
                ActorError::Stopped
            })
    }

    /// アクターの現在の状態を取得します（実装はオプション、または別の方法で提供される可能性あり）。
    // pub async fn ask_state(&self) -> Result<A::State, ActorError> { ... }

    /// アクターの停止を試みます。
    // pub async fn stop(&self) -> Result<(), ActorError> { ... }

    // 内部的なコンストラクタ（アクターシステムなどが使用）
    // Sender を受け取るように変更
    pub(crate) fn new(id: String, sender: mpsc::Sender<A::Event>) -> Self {
        Self {
            id,
            sender,
        }
    }

    /// アクター参照のIDを取得します。
    pub fn id(&self) -> &str {
        &self.id
    }
}

// ActorRef が Send と Sync であることは、mpsc::Sender が A::Event が Send であれば
// Send + Sync であることに依存します。Actor トレイトの境界により A::Event は Send + Sync なので
// ActorRef も自動的に Send + Sync になります。unsafe impl は不要です。
// unsafe impl<A: Actor> Send for ActorRef<A> {}
// unsafe impl<A: Actor> Sync for ActorRef<A> {} 