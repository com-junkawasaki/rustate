use rustate_grpc::server::{StateMachineServiceServer, service::RuStateMachineService};
use tonic::transport::Server;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ポートを環境変数から取得するか、デフォルトを使用
    let port = env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("[::1]:{}", port).parse()?;
    
    let service = RuStateMachineService::new();
    
    println!("RuState gRPCサーバーを起動中: {}", addr);
    
    Server::builder()
        .add_service(StateMachineServiceServer::new(service))
        .serve(addr)
        .await?;
    
    Ok(())
} 