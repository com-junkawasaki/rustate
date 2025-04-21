/// RuState gRPC モジュール
/// 
/// このモジュールは、RuStateステートマシンをgRPC経由でネットワークに公開するための
/// インターフェースを提供します。サーバー側とクライアント側の両方の実装を含みます。
/// 
/// # 主な機能
/// 
/// - 型安全なステートマシンの定義と操作
/// - リアルタイムの状態変化監視（ストリーミング）
/// - バッチ処理によるトランザクション的イベント処理
/// - 型安全なクライアントコードの動的生成
/// 
/// # 使用例
/// 
/// ## サーバー側
/// 
/// ```rust,no_run
/// use rustate_grpc::server::{StateMachineServiceServer, StateMachineService};
/// use rustate_grpc::server::service::RuStateMachineService;
/// use tonic::transport::Server;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let addr = "[::1]:50051".parse()?;
///     let service = RuStateMachineService::new();
///     
///     Server::builder()
///         .add_service(StateMachineServiceServer::new(service))
///         .serve(addr)
///         .await?;
///     
///     Ok(())
/// }
/// ```
/// 
/// ## クライアント側
/// 
/// ```rust,no_run
/// use rustate_grpc::client::StateMachineServiceClient;
/// use rustate_grpc::client::types::{CreateMachineRequest, SendEventRequest, EventDefinition};
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = StateMachineServiceClient::connect("http://[::1]:50051").await?;
///     
///     // ステートマシンの定義と作成
///     let request = CreateMachineRequest { /* ... */ };
///     let response = client.create_machine(request).await?;
///     
///     // イベントの送信
///     let event = EventDefinition {
///         machine_id: response.get_ref().machine_id.clone(),
///         event_type: "TIMER".to_string(),
///         payload: "{}".to_string(),
///     };
///     
///     let request = SendEventRequest { event: Some(event) };
///     let response = client.send_event(request).await?;
///     
///     println!("Event processed: {:?}", response);
///     
///     Ok(())
/// }
/// ```

pub mod error;

// Protoから生成された型定義
pub mod proto {
    tonic::include_proto!("rustate");
}

// サーバー側の実装
#[cfg(feature = "server")]
pub mod server;

// クライアント側の実装
#[cfg(feature = "client")]
pub mod client;

// 共通のコンバーター
pub mod converter;

// パブリックAPI
pub use proto as types;

/// gRPCサーバーを起動するためのユーティリティ関数
/// 
/// # 引数
/// 
/// * `addr` - サーバーをバインドするアドレス
/// 
/// # 例
/// 
/// ```rust,no_run
/// use rustate_grpc::run_server;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     run_server("[::1]:50051").await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "server")]
pub async fn run_server<A>(addr: A) -> Result<(), tonic::transport::Error>
where
    A: std::net::ToSocketAddrs,
{
    use server::{StateMachineServiceServer, service::RuStateMachineService};
    use tonic::transport::Server;
    use std::net::SocketAddr;

    let service = RuStateMachineService::new();
    
    // SocketAddrsをイテレート
    for addr in addr.to_socket_addrs().expect("Invalid address") {
        let addr = addr;
        return Server::builder()
            .add_service(StateMachineServiceServer::new(service))
            .serve(addr)
            .await;
    }
    
    Err(tonic::transport::Error::new_invalid_uri())
} 