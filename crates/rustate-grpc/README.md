# RuState gRPC

RuStateのgRPCインターフェース。ネットワーク上で型安全なステートマシンの状態制御を可能にします。

## 特徴

- **型安全なステートマシン操作**: RuStateの全機能をgRPC経由で利用可能
- **リアルタイム状態変化監視**: ストリーミングでステートマシンの状態変化をリアルタイム監視
- **バッチ処理**: 複数イベントのトランザクション的処理
- **自動コード生成**: クライアント側の型安全コードを動的生成
- **クロスプラットフォーム**: 様々な言語・環境からアクセス可能

## インストール

```toml
# Cargo.toml
[dependencies]
rustate-grpc = { version = "0.1.0", features = ["full"] }
```

## サーバー側の使用例

```rust
use rustate_grpc::run_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // サーバーをポート50051で起動
    run_server("[::1]:50051").await?;
    
    Ok(())
}
```

あるいは、より詳細な設定:

```rust
use rustate_grpc::server::{StateMachineServiceServer, service::RuStateMachineService};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = RuStateMachineService::new();
    
    println!("サーバーを起動中: {}", addr);
    
    Server::builder()
        .add_service(StateMachineServiceServer::new(service))
        .serve(addr)
        .await?;
    
    Ok(())
}
```

## クライアント側の使用例

### 基本的な使用法

```rust
use rustate_grpc::client::StateMachineServiceClient;
use rustate_grpc::types::{
    CreateMachineRequest, MachineDefinition, State, Transition, 
    SendEventRequest, EventDefinition, StateType
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // クライアントの作成
    let mut client = StateMachineServiceClient::connect("http://[::1]:50051").await?;
    
    // ステートマシンの定義
    let machine_def = MachineDefinition {
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
        context: "{}".to_string(),
    };
    
    // ステートマシンの作成
    let create_request = CreateMachineRequest {
        definition: Some(machine_def),
    };
    
    let response = client.create_machine(create_request).await?;
    let machine_id = response.get_ref().machine_id.clone();
    
    println!("ステートマシンを作成しました: {}", machine_id);
    println!("初期状態: {:?}", response.get_ref().initial_state);
    
    // イベントの送信
    let event = EventDefinition {
        machine_id: machine_id.clone(),
        event_type: "TIMER".to_string(),
        payload: "{}".to_string(),
    };
    
    let send_request = SendEventRequest {
        event: Some(event),
    };
    
    let response = client.send_event(send_request).await?;
    
    println!("イベント処理結果: {:?}", response.get_ref().result);
    
    // 状態の監視
    let watch_request = rustate_grpc::types::WatchMachineRequest {
        machine_id: machine_id.clone(),
        include_context: true,
    };
    
    let mut stream = client.watch_machine(watch_request).await?.into_inner();
    
    println!("状態変更の監視中...");
    
    // 別のスレッドでイベントを送信
    let client_clone = client.clone();
    let machine_id_clone = machine_id.clone();
    
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        for _ in 0..3 {
            let event = EventDefinition {
                machine_id: machine_id_clone.clone(),
                event_type: "TIMER".to_string(),
                payload: "{}".to_string(),
            };
            
            let send_request = SendEventRequest {
                event: Some(event),
            };
            
            let _ = client_clone.clone().send_event(send_request).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    
    // ストリーミングで状態変更を受信
    while let Some(change) = stream.message().await? {
        println!("状態変更検知: {:?}", change);
    }
    
    Ok(())
}
```

### 型安全なクライアント

```rust
use rustate_grpc::client::typesafe::TypeSafeClient;
use std::fmt;

// イベント型を定義
#[derive(Clone)]
enum TrafficLightEvent {
    Timer,
    Emergency,
    PowerOff,
}

impl fmt::Display for TrafficLightEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer => write!(f, "TIMER"),
            Self::Emergency => write!(f, "EMERGENCY"),
            Self::PowerOff => write!(f, "POWER_OFF"),
        }
    }
}

// コンテキスト型を定義
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
struct TrafficLightContext {
    timer_value: u32,
    is_emergency: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 型安全なクライアントを作成
    let mut client = TypeSafeClient::<TrafficLightEvent, TrafficLightContext>::new(
        "http://[::1]:50051",
        "traffic_light",
    ).await?;
    
    // イベント送信
    let result = client.send_event(TrafficLightEvent::Timer).await?;
    
    println!("イベント処理結果: {:?}", result);
    println!("コンテキスト: {:?}", result.context);
    
    // 現在の状態を取得
    let state = client.get_state().await?;
    
    println!("現在の状態: {:?}", state.current_states);
    println!("コンテキスト: {:?}", state.context);
    
    // バッチ処理
    let events = vec![
        TrafficLightEvent::Timer,
        TrafficLightEvent::Emergency,
        TrafficLightEvent::Timer,
    ];
    
    let batch_result = client.batch_events(events).await?;
    
    println!("バッチ処理結果: {:?}", batch_result);
    
    Ok(())
}
```

### トランスポート設定

```rust
use rustate_grpc::client::{
    StateMachineServiceClient,
    transport::{ClientTransport, TransportConfig},
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // カスタムトランスポート設定
    let config = TransportConfig {
        connect_timeout: Some(Duration::from_secs(5)),
        timeout: Some(Duration::from_secs(20)),
        concurrency_limit: Some(512),
        user_agent: Some("my-app/1.0".to_string()),
        // その他の設定...
        ..Default::default()
    };
    
    // トランスポートの設定
    let transport = ClientTransport::new("http://[::1]:50051")?
        .with_config(&config);
    
    // チャネルを作成
    let channel = transport.connect().await?;
    
    // クライアントを作成
    let client = StateMachineServiceClient::new(channel);
    
    // ... クライアントの使用 ...
    
    Ok(())
}
```

## セキュリティ

本番環境では、以下のセキュリティ対策を検討してください：

1. **TLS暗号化**: 機密データを保護するためにTLS/SSLを使用
2. **認証**: gRPCの認証機能（JWT、OAuth2など）を利用
3. **認可**: タスクごとに必要最小限の権限を設定
4. **レート制限**: サービス妨害攻撃を防ぐためのレート制限
5. **ロギング**: セキュリティ監査のための詳細なログ記録

## ライセンス

MIT 