pub mod actor;
pub mod actor_ref;
pub mod logic;
pub mod simple_counter;
pub mod spawn;

// spawn 関数をトップレベルで公開
pub use spawn::spawn;

#[cfg(test)]
mod tests {
    use super::*;
    use simple_counter::{CounterActor, CounterEvent};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_counter_actor() {
        println!("Spawning CounterActor...");
        let counter_ref = spawn(CounterActor::default());
        println!("CounterActor spawned with ref: {:?}", counter_ref);

        // 少し待機してアクターが初期化される時間を与える（デバッグ用）
        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event...");
        let res1 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res1.is_ok());
        println!("Increment event sent.");

        // 状態が更新されるのを少し待つ
        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event again...");
        let res2 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res2.is_ok());
        println!("Increment event sent.");

        sleep(Duration::from_millis(10)).await;

        println!("Sending Print event...");
        let res3 = counter_ref.send(CounterEvent::Print).await;
        assert!(res3.is_ok());
        println!("Print event sent.");

        // アクターがイベントを処理するのを待つ
        sleep(Duration::from_millis(50)).await;

        // ここでは println! の出力を目視で確認する
        // 将来的には状態を取得するメカニズムが必要
        println!("Test finished. Check logs for actor output.");
    }
} 