use rustate_grpc::client::StateMachineServiceClient;
use rustate_grpc::types::{
    CreateMachineRequest, EventDefinition, GetStateRequest, MachineDefinition, SendEventRequest,
    State, StateType, Transition, WatchMachineRequest,
};
use std::env;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // サーバーアドレスを環境変数から取得するか、デフォルトを使用
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "http://[::1]:50051".to_string());

    println!("サーバーに接続中: {}", server_addr);

    // クライアントの作成
    let mut client = StateMachineServiceClient::connect(server_addr).await?;
    println!("接続完了");

    // トラフィックライトのステートマシンを定義
    println!("トラフィックライトのステートマシンを定義中...");
    let machine_def = create_traffic_light_machine();

    // ステートマシンの作成
    let create_request = CreateMachineRequest {
        definition: Some(machine_def),
    };

    println!("ステートマシンを作成中...");
    let response = client.create_machine(create_request).await?;
    let machine_id = response.get_ref().machine_id.clone();

    println!("ステートマシンを作成しました: {}", machine_id);

    if let Some(initial_state) = &response.get_ref().initial_state {
        println!("初期状態: {}", initial_state.current_states.join(", "));
    }

    // 状態変化の監視を開始
    println!("状態変化の監視を開始...");
    watch_machine_states(&mut client, &machine_id).await?;

    Ok(())
}

// トラフィックライトのステートマシン定義
fn create_traffic_light_machine() -> MachineDefinition {
    MachineDefinition {
        id: "traffic_light".to_string(),
        initial: "green".to_string(),
        states: vec![
            State {
                id: "green".to_string(),
                r#type: StateType::Normal as i32,
                parent: "".to_string(),
                children: vec![],
            },
            State {
                id: "yellow".to_string(),
                r#type: StateType::Normal as i32,
                parent: "".to_string(),
                children: vec![],
            },
            State {
                id: "red".to_string(),
                r#type: StateType::Normal as i32,
                parent: "".to_string(),
                children: vec![],
            },
        ],
        transitions: vec![
            Transition {
                source: "green".to_string(),
                event: "TIMER".to_string(),
                target: "yellow".to_string(),
                guards: vec![],
                actions: vec![],
            },
            Transition {
                source: "yellow".to_string(),
                event: "TIMER".to_string(),
                target: "red".to_string(),
                guards: vec![],
                actions: vec![],
            },
            Transition {
                source: "red".to_string(),
                event: "TIMER".to_string(),
                target: "green".to_string(),
                guards: vec![],
                actions: vec![],
            },
        ],
        actions: vec![],
        guards: vec![],
        context: r#"{"counter": 0}"#.to_string(),
    }
}

// 状態変化の監視
async fn watch_machine_states(
    client: &mut StateMachineServiceClient<tonic::transport::Channel>,
    machine_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 状態変化監視リクエスト
    let watch_request = WatchMachineRequest {
        machine_id: machine_id.to_string(),
        include_context: true,
    };

    // 状態変化ストリームを取得
    let mut stream = client.watch_machine(watch_request).await?.into_inner();

    // 別のタスクでイベント送信
    let client_clone = client.clone();
    let machine_id_clone = machine_id.to_string();

    tokio::spawn(async move {
        // 最初のイベント前に少し待機
        tokio::time::sleep(Duration::from_secs(1)).await;

        for i in 0..6 {
            // イベント送信
            let event = EventDefinition {
                machine_id: machine_id_clone.clone(),
                event_type: "TIMER".to_string(),
                payload: format!(r#"{{"iteration": {}}}"#, i).to_string(),
            };

            let request = SendEventRequest { event: Some(event) };

            println!("イベント送信中 (TIMER, iteration={})", i);

            match client_clone.clone().send_event(request).await {
                Ok(_) => println!("イベント送信成功"),
                Err(e) => eprintln!("イベント送信エラー: {}", e),
            }

            // 次のイベントまで待機
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        println!("イベント送信完了");
    });

    // ストリームからの応答を処理
    println!("状態変更の監視中...");

    let mut count = 0;
    while let Some(change) = stream.message().await? {
        count += 1;

        println!("\n状態変更 #{}: ", count);
        println!("  イベント: {}", change.event_type);
        println!("  前の状態: {}", change.previous_states.join(", "));
        println!("  現在の状態: {}", change.current_states.join(", "));
        println!("  コンテキスト: {}", change.context);
        println!("  タイムスタンプ: {}", change.timestamp);

        // 6つの状態変化を受け取ったら終了
        if count >= 6 {
            println!("\n6つの状態変化を検出しました。監視を終了します。");
            break;
        }
    }

    Ok(())
}
