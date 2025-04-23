use crate::actor::{Actor, ActorError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// カウンターの状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterState {
    pub count: i32,
}

/// カウンターが受け付けるイベント
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CounterEvent {
    Increment,
    Decrement,
    // Get イベントは状態を返す必要があるため、今の Actor::receive シグネチャでは
    // 直接扱いにくい。デモンストレーションのため、今回は print するだけにする。
    Print,
}

/// カウンターアクターの構造体（ロジックを持つ）
#[derive(Debug, Clone, Default)] // Default は initial_state のために便利
pub struct CounterActor;

#[async_trait]
impl Actor for CounterActor {
    type State = CounterState;
    type Event = CounterEvent;
    // このアクターは外部に特定の出力を生成しないので () とする
    type Output = ();

    fn initial_state(&self) -> Self::State {
        CounterState { count: 0 }
    }

    async fn receive(
        &self,
        mut state: Self::State, // 可変にするために mut を追加
        event: Self::Event,
    ) -> Result<Self::State, ActorError> {
        println!(
            "CounterActor received event: {:?}, current state: {:?}",
            event, state
        );
        match event {
            CounterEvent::Increment => {
                state.count += 1;
                Ok(state)
            }
            CounterEvent::Decrement => {
                state.count -= 1;
                Ok(state)
            }
            CounterEvent::Print => {
                println!("Current count: {}", state.count);
                // 状態は変化しないので、そのまま返す
                Ok(state)
            }
        }
    }
}
