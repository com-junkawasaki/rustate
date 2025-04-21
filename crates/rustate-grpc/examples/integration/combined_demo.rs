// rustate-grpc 統合デモ
// このサンプルは、rustateのステートマシンをgRPC経由で制御する方法を示します
//
// 以下のコマンドで実行：
// cargo run --example combined_demo --features=full

use rustate::{
    Action, ActionType, Machine, MachineBuilder, State, Transition
};
use rustate_grpc::{
    client::StateMachineServiceClient,
    types::{
        CreateMachineRequest, MachineDefinition, StateType,
        SendEventRequest, EventDefinition, WatchMachineRequest
    },
    converter::machine_to_proto,
    run_server
};
use std::time::Duration;
use std::{sync::Arc, error::Error};
use tokio::sync::Mutex;

// 典型的なステートマシンを作成（トラフィックライト）
fn create_traffic_light_machine() -> Machine {
    let green = State::new("green");
    let yellow = State::new("yellow");
    let red = State::new("red");

    let change_to_yellow = Action::new(
        "changeToYellow",
        ActionType::Transition,
        |ctx, _| {
            println!("緑→黄色に変化中");
            let _ = ctx.set("last_transition", "green_to_yellow");
        }
    );

    let change_to_red = Action::new(
        "changeToRed",
        ActionType::Transition,
        |ctx, _| {
            println!("黄色→赤に変化中");
            let _ = ctx.set("last_transition", "yellow_to_red");
        }
    );

    let change_to_green = Action::new(
        "changeToGreen",
        ActionType::Transition,
        |ctx, _| {
            println!("赤→緑に変化中");
            let _ = ctx.set("last_transition", "red_to_green");
        }
    );

    let machine = MachineBuilder::new("traffic_light")
        .state(green)
        .state(yellow)
        .state(red)
        .initial("green")
        .transition(
            Transition::new("green", "TIMER", "yellow")
                .with_action("changeToYellow")
        )
        .transition(
            Transition::new("yellow", "TIMER", "red")
                .with_action("changeToRed")
        )
        .transition(
            Transition::new("red", "TIMER", "green")
                .with_action("changeToGreen")
        )
        .action(change_to_yellow)
        .action(change_to_red)
        .action(change_to_green)
        .context(serde_json::json!({
            "last_transition": "none",
            "cycle_count": 0
        }))
        .build()
        .unwrap();

    machine
}

// サーバーとクライアントを統合的に実行するデモ
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // サーバーのアドレス
    let addr = "[::1]:50052";
    
    // サーバーを別スレッドで起動
    let server_handle = {
        let addr_clone = addr.to_string();
        tokio::spawn(async move {
            println!("gRPCサーバーを起動: {}", addr_clone);
            run_server(&addr_clone).await.unwrap();
        })
    };
    
    // サーバー起動を少し待つ
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // クライアントの作成
    let mut client = StateMachineServiceClient::connect(format!("http://{}", addr)).await?;
    println!("gRPCクライアント接続完了");
    
    // 1. ローカルでステートマシンを作成
    let local_machine = create_traffic_light_machine();
    println!("ローカルマシン作成完了: {}", local_machine.name);
    
    // 2. gRPC経由でサーバーに同様のマシンを作成
    // ローカルマシンをProtobuf形式に変換
    let machine_def = machine_to_proto(&local_machine)?;
    
    // リモートマシンを作成
    let create_request = CreateMachineRequest {
        definition: Some(machine_def),
    };
    
    let response = client.create_machine(create_request).await?;
    let remote_machine_id = response.get_ref().machine_id.clone();
    
    println!("リモートマシン作成完了: {}", remote_machine_id);
    println!("初期状態: {:?}", response.get_ref().initial_state);
    
    // 3. 状態変化の監視を開始
    let watch_request = WatchMachineRequest {
        machine_id: remote_machine_id.clone(),
        include_context: true,
    };
    
    let mut state_stream = client.watch_machine(watch_request).await?.into_inner();
    
    // 監視ストリームを別スレッドで処理
    let monitoring_handle = tokio::spawn(async move {
        println!("状態変化の監視を開始");
        
        while let Some(state_change) = state_stream.message().await.unwrap() {
            println!("状態変化検知:");
            println!("  状態: {:?}", state_change.current_state);
            
            if let Some(context) = state_change.context {
                println!("  コンテキスト: {}", context);
            }
        }
    });
    
    // 4. イベント送信のループ
    for i in 1..=5 {
        // ローカルマシンでイベント処理
        println!("\n----- サイクル {} -----", i);
        println!("ローカルマシン状態（前）: {:?}", local_machine.current_states);
        let _ = local_machine.send("TIMER", serde_json::json!({}));
        println!("ローカルマシン状態（後）: {:?}", local_machine.current_states);
        
        // リモートマシンにイベント送信
        let event = EventDefinition {
            machine_id: remote_machine_id.clone(),
            event_type: "TIMER".to_string(),
            payload: "{}".to_string(),
        };
        
        let send_request = SendEventRequest {
            event: Some(event),
        };
        
        println!("リモートマシンにTIMERイベント送信中...");
        let result = client.send_event(send_request).await?;
        println!("送信結果: {:?}", result.get_ref().result);
        
        // 少し待つ
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    
    // 少し待ってからクリーンアップ
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("\nデモ完了。クリーンアップ中...");
    
    // Ctrl+Cをエミュレート
    tokio::signal::ctrl_c().await?;
    
    Ok(())
}