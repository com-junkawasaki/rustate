use rustate_grpc::client::typesafe::TypeSafeClient;
use rustate_grpc::client::StateMachineServiceClient;
use rustate_grpc::types::{
    CreateMachineRequest, MachineDefinition, State, Transition, StateType,
};
use std::env;
use std::fmt;
use serde::{Serialize, Deserialize};

// トラフィックライトのイベント型を定義
#[derive(Clone, Debug)]
enum TrafficLightEvent {
    Timer,
    Emergency,
    Reset,
}

// イベントの文字列表現
impl fmt::Display for TrafficLightEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer => write!(f, "TIMER"),
            Self::Emergency => write!(f, "EMERGENCY"),
            Self::Reset => write!(f, "RESET"),
        }
    }
}

// トラフィックライトのコンテキスト型を定義
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
struct TrafficLightContext {
    counter: u32,
    is_emergency: bool,
    last_event: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // サーバーアドレスを環境変数から取得するか、デフォルトを使用
    let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "http://[::1]:50051".to_string());
    
    println!("サーバーに接続中: {}", server_addr);
    
    // 通常のクライアントを作成
    let mut client = StateMachineServiceClient::connect(&server_addr).await?;
    println!("接続完了");
    
    // ステートマシンの作成
    println!("トラフィックライトのステートマシンを作成中...");
    let machine_id = create_machine(&mut client).await?;
    
    // 型安全なクライアントを作成
    let mut typesafe_client = TypeSafeClient::<TrafficLightEvent, TrafficLightContext>::new(
        &server_addr,
        &machine_id,
    ).await?;
    
    println!("\n型安全なクライアントを使用してイベントを送信します");
    
    // イベント送信
    println!("\nTIMERイベントを送信中...");
    let result = typesafe_client.send_event(TrafficLightEvent::Timer).await?;
    println!("TIMER処理結果:");
    println!("  成功: {}", result.success);
    println!("  状態変化: {}", result.states_changed.join(" -> "));
    println!("  コンテキスト: {:?}", result.context);
    
    // 現在の状態を取得
    println!("\n現在の状態を取得中...");
    let state = typesafe_client.get_state().await?;
    println!("現在の状態:");
    println!("  マシンID: {}", state.machine_id);
    println!("  状態: {}", state.current_states.join(", "));
    println!("  コンテキスト: {:?}", state.context);
    
    // 緊急イベントを送信
    println!("\nEMERGENCYイベントを送信中...");
    let result = typesafe_client.send_event(TrafficLightEvent::Emergency).await?;
    println!("EMERGENCY処理結果:");
    println!("  成功: {}", result.success);
    println!("  状態変化: {}", result.states_changed.join(" -> "));
    println!("  コンテキスト: {:?}", result.context);
    
    // バッチ処理の例
    println!("\n複数イベントをバッチ処理中...");
    let events = vec![
        TrafficLightEvent::Reset,
        TrafficLightEvent::Timer,
        TrafficLightEvent::Timer,
    ];
    
    let batch_result = typesafe_client.batch_events(events).await?;
    
    println!("バッチ処理結果:");
    for (i, result) in batch_result.results.iter().enumerate() {
        println!("  イベント #{}: 成功={}, 状態={}", i + 1, result.success, result.states_changed.join(", "));
    }
    
    if let Some(final_state) = batch_result.final_state {
        println!("\n最終状態:");
        println!("  状態: {}", final_state.current_states.join(", "));
        println!("  コンテキスト: {:?}", final_state.context);
    }
    
    Ok(())
}

// 通常のクライアントを使用してステートマシンを作成
async fn create_machine(
    client: &mut StateMachineServiceClient<tonic::transport::Channel>,
) -> Result<String, Box<dyn std::error::Error>> {
    // トラフィックライトのステートマシンを定義
    let machine_def = MachineDefinition {
        id: format!("traffic_light_{}", chrono::Utc::now().timestamp_millis()),
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
            // 通常の遷移
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
            // 緊急遷移
            Transition {
                source: "green".to_string(),
                event: "EMERGENCY".to_string(),
                target: "red".to_string(),
                guards: vec![],
                actions: vec![],
            },
            Transition {
                source: "yellow".to_string(),
                event: "EMERGENCY".to_string(),
                target: "red".to_string(),
                guards: vec![],
                actions: vec![],
            },
            // リセット遷移
            Transition {
                source: "red".to_string(),
                event: "RESET".to_string(),
                target: "green".to_string(),
                guards: vec![],
                actions: vec![],
            },
        ],
        actions: vec![],
        guards: vec![],
        context: r#"{"counter": 0, "is_emergency": false, "last_event": ""}"#.to_string(),
    };
    
    // ステートマシンの作成
    let create_request = CreateMachineRequest {
        definition: Some(machine_def),
    };
    
    let response = client.create_machine(create_request).await?;
    let machine_id = response.get_ref().machine_id.clone();
    
    println!("ステートマシンを作成しました: {}", machine_id);
    
    if let Some(initial_state) = &response.get_ref().initial_state {
        println!("初期状態: {}", initial_state.current_states.join(", "));
    }
    
    Ok(machine_id)
} 