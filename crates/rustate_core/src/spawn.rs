use crate::actor::{Actor, ActorError};
use crate::actor_ref::ActorRef;
use tokio::sync::mpsc;
use uuid::Uuid;

pub const DEFAULT_BUFFER_SIZE: usize = 32; // pub を追加

/// アクターを生成し、独立した非同期タスクで実行を開始します。
///
/// # Arguments
/// * `actor` - `Actor` トレイトを実装したアクターインスタンス。
/// * `buffer` - アクターのイベントキュー（メールボックス）のサイズ。
///
/// # Returns
/// 生成されたアクターへの参照 (`ActorRef`)。
pub fn spawn_actor<A: Actor>(actor: A, buffer: usize) -> ActorRef<A>
where
    A::State: PartialEq,
{
    let id = Uuid::new_v4().to_string();
    let (sender, mut receiver) = mpsc::channel::<A::Event>(buffer);

    let actor_ref = ActorRef::new(id.clone(), sender);

    tokio::spawn(async move {
        let mut current_state = actor.initial_state();
        println!(
            "Actor {} spawned with initial state: {:?}",
            id, current_state
        );

        while let Some(event) = receiver.recv().await {
            match actor.receive(current_state.clone(), event).await {
                Ok(new_state) => {
                    // 状態が変化した場合にログを出力（オプション）
                    if new_state != current_state {
                        println!(
                            "Actor {} state changed: {:?} -> {:?}",
                            id, current_state, new_state
                        );
                        current_state = new_state;
                    } else {
                         println!(
                            "Actor {} state unchanged: {:?}",
                            id, current_state
                        );
                    }
                }
                Err(err) => {
                    eprintln!("Actor {} error processing event: {}", id, err);
                    // エラーの種類に応じて処理を分岐させることも可能
                    // 例えば ActorError::Stopped ならループを抜けるなど
                    if matches!(err, ActorError::Stopped) {
                        eprintln!("Actor {} stopping due to error.", id);
                        break;
                    }
                    // 他のエラーの場合は処理を続けるか、停止するかなどを決定
                }
            }
        }
        println!("Actor {} task finished.", id);
        // receiver が閉じられたら（対応する ActorRef がすべてドロップされたら）
        // ループが終了し、タスクも完了する。
    });

    actor_ref
}

/// デフォルトのバッファサイズでアクターを生成します。
pub fn spawn<A: Actor>(actor: A) -> ActorRef<A>
where
    A::State: PartialEq,
{
    spawn_actor(actor, DEFAULT_BUFFER_SIZE)
} 