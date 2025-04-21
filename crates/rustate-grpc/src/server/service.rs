use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use chrono::Utc;

use rustate::Machine;
use crate::converter;
use crate::error::{GrpcError, Result};
use crate::proto::{
    self, CreateMachineRequest, CreateMachineResponse, SendEventRequest, SendEventResponse,
    BatchEventsRequest, BatchEventsResponse, GetStateRequest, MachineState,
    UnregisterMachineRequest, UnregisterMachineResponse, WatchMachineRequest,
    StateChangeEvent, GenerateClientCodeRequest, GenerateClientCodeResponse,
};
use crate::proto::state_machine_service_server::StateMachineService;

// 状態変更通知用のチャネル型
type StateChangeChannel = mpsc::Sender<Result<StateChangeEvent, Status>>;

/// RuStateステートマシンサービスの実装
pub struct RuStateMachineService {
    // ステートマシンレジストリ
    machines: Arc<RwLock<HashMap<String, Machine>>>,
    // マシンID -> 監視チャネルのマッピング
    watchers: Arc<RwLock<HashMap<String, Vec<StateChangeChannel>>>>,
}

impl RuStateMachineService {
    pub fn new() -> Self {
        Self {
            machines: Arc::new(RwLock::new(HashMap::new())),
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // 状態変更通知をすべての監視者に送信
    fn notify_state_change(
        &self,
        machine_id: &str,
        event_type: &str,
        previous_states: Vec<String>,
        current_states: Vec<String>,
        context: String,
    ) {
        let machine_id = machine_id.to_string();
        let event_type = event_type.to_string();
        let timestamp = Utc::now().to_rfc3339();
        
        let event = StateChangeEvent {
            machine_id: machine_id.clone(),
            event_type,
            previous_states,
            current_states,
            context,
            timestamp,
        };
        
        // 監視チャネルを取得
        let watchers = self.watchers.read().unwrap();
        if let Some(channels) = watchers.get(&machine_id) {
            // すべての監視者に通知
            for channel in channels {
                let event_clone = event.clone();
                // チャネルが閉じていることは無視（クリーンアップは別途行う）
                let _ = channel.try_send(Ok(event_clone));
            }
        }
    }
}

#[tonic::async_trait]
impl StateMachineService for RuStateMachineService {
    /// ステートマシンの作成と登録
    async fn create_machine(
        &self,
        request: Request<CreateMachineRequest>,
    ) -> std::result::Result<Response<CreateMachineResponse>, Status> {
        let req = request.into_inner();
        let Some(definition) = req.definition else {
            return Err(Status::invalid_argument("Machine definition is required"));
        };
        
        // protoからRuState形式に変換
        let machine = converter::machine_from_proto(&definition)
            .map_err(Status::from)?;
        
        let machine_id = machine.id().to_string();
        
        // ステートマシンの状態をproto形式に変換
        let initial_state = converter::machine_state_to_proto(&machine)
            .map_err(Status::from)?;
        
        // レジストリに登録
        let mut machines = self.machines.write().unwrap();
        machines.insert(machine_id.clone(), machine);
        
        // 監視リスト作成
        let mut watchers = self.watchers.write().unwrap();
        watchers.entry(machine_id.clone()).or_insert_with(Vec::new);
        
        // レスポンス作成
        let response = CreateMachineResponse {
            machine_id,
            initial_state: Some(initial_state),
        };
        
        Ok(Response::new(response))
    }

    /// イベントの送信
    async fn send_event(
        &self,
        request: Request<SendEventRequest>,
    ) -> std::result::Result<Response<SendEventResponse>, Status> {
        let req = request.into_inner();
        let Some(event) = req.event else {
            return Err(Status::invalid_argument("Event is required"));
        };
        
        let machine_id = event.machine_id.clone();
        let event_type = event.event_type.clone();
        
        // マシンレジストリからステートマシンを取得
        let mut machines = self.machines.write().unwrap();
        let machine = machines.get_mut(&machine_id).ok_or_else(|| {
            Status::not_found(format!("Machine not found: {}", machine_id))
        })?;
        
        // 現在の状態を保存（通知用）
        let previous_states = machine.current_states().iter().map(|s| s.to_string()).collect();
        
        // イベントを送信
        let event_name = converter::event_from_proto(&event).map_err(Status::from)?;
        let success = machine.send(&event_name).map_err(|e| {
            Status::internal(format!("Failed to process event: {}", e))
        })?;
        
        // 更新後の状態
        let current_states = machine.current_states().iter().map(|s| s.to_string()).collect();
        
        // コンテキストをJSON文字列化
        let context_json = serde_json::to_string(machine.context()).map_err(|e| {
            Status::internal(format!("Failed to serialize context: {}", e))
        })?;
        
        // 状態変更通知
        self.notify_state_change(
            &machine_id,
            &event_type,
            previous_states,
            current_states.clone(),
            context_json.clone(),
        );
        
        // レスポンス作成
        let result = proto::EventResult {
            success,
            states_changed: current_states,
            context: context_json,
            error_message: "".to_string(),
        };
        
        let response = SendEventResponse {
            result: Some(result),
        };
        
        Ok(Response::new(response))
    }

    /// ステートマシンの現在の状態を取得
    async fn get_state(
        &self,
        request: Request<GetStateRequest>,
    ) -> std::result::Result<Response<MachineState>, Status> {
        let req = request.into_inner();
        let machine_id = req.machine_id;
        
        // マシンレジストリからステートマシンを取得
        let machines = self.machines.read().unwrap();
        let machine = machines.get(&machine_id).ok_or_else(|| {
            Status::not_found(format!("Machine not found: {}", machine_id))
        })?;
        
        // ステートマシンの状態を変換
        let state = converter::machine_state_to_proto(machine)
            .map_err(Status::from)?;
        
        Ok(Response::new(state))
    }

    /// 複数イベントをバッチ処理
    async fn batch_events(
        &self,
        request: Request<BatchEventsRequest>,
    ) -> std::result::Result<Response<BatchEventsResponse>, Status> {
        let req = request.into_inner();
        let machine_id = req.machine_id.clone();
        
        // マシンレジストリからステートマシンを取得
        let mut machines = self.machines.write().unwrap();
        let machine = machines.get_mut(&machine_id).ok_or_else(|| {
            Status::not_found(format!("Machine not found: {}", machine_id))
        })?;
        
        // 各イベントを処理
        let mut results = Vec::new();
        
        for event in req.events {
            // 現在の状態を保存（通知用）
            let previous_states = machine.current_states().iter().map(|s| s.to_string()).collect();
            
            // イベントを送信
            let event_name = converter::event_from_proto(&event).map_err(Status::from)?;
            let success = match machine.send(&event_name) {
                Ok(success) => {
                    let current_states = machine.current_states().iter().map(|s| s.to_string()).collect();
                    
                    // コンテキストをJSON文字列化
                    let context_json = serde_json::to_string(machine.context()).map_err(|e| {
                        Status::internal(format!("Failed to serialize context: {}", e))
                    })?;
                    
                    // 状態変更通知
                    self.notify_state_change(
                        &machine_id,
                        &event.event_type,
                        previous_states,
                        current_states.clone(),
                        context_json.clone(),
                    );
                    
                    // 結果を追加
                    let result = proto::EventResult {
                        success,
                        states_changed: current_states,
                        context: context_json,
                        error_message: "".to_string(),
                    };
                    
                    results.push(result);
                    success
                }
                Err(e) => {
                    // エラー結果を追加
                    let result = proto::EventResult {
                        success: false,
                        states_changed: vec![],
                        context: "{}".to_string(),
                        error_message: format!("{}", e),
                    };
                    
                    results.push(result);
                    false
                }
            };
            
            // イベント処理が失敗した場合はバッチ処理を中断
            if !success {
                break;
            }
        }
        
        // 最終状態を取得
        let final_state = converter::machine_state_to_proto(machine)
            .map_err(Status::from)?;
        
        // レスポンス作成
        let response = BatchEventsResponse {
            results,
            final_state: Some(final_state),
        };
        
        Ok(Response::new(response))
    }

    /// ステートマシンの登録解除
    async fn unregister_machine(
        &self,
        request: Request<UnregisterMachineRequest>,
    ) -> std::result::Result<Response<UnregisterMachineResponse>, Status> {
        let req = request.into_inner();
        let machine_id = req.machine_id;
        
        // マシンレジストリからステートマシンを削除
        let mut machines = self.machines.write().unwrap();
        let removed = machines.remove(&machine_id).is_some();
        
        // 監視リストからも削除
        if removed {
            let mut watchers = self.watchers.write().unwrap();
            watchers.remove(&machine_id);
        }
        
        // レスポンス作成
        let response = UnregisterMachineResponse {
            success: removed,
            message: if removed {
                format!("Machine {} unregistered", machine_id)
            } else {
                format!("Machine {} not found", machine_id)
            },
        };
        
        Ok(Response::new(response))
    }

    /// ステートマシンの状態変化を監視（ストリーミング）
    type WatchMachineStream = ReceiverStream<Result<StateChangeEvent, Status>>;
    
    async fn watch_machine(
        &self,
        request: Request<WatchMachineRequest>,
    ) -> std::result::Result<Response<Self::WatchMachineStream>, Status> {
        let req = request.into_inner();
        let machine_id = req.machine_id;
        
        // マシンの存在確認
        let machines = self.machines.read().unwrap();
        if !machines.contains_key(&machine_id) {
            return Err(Status::not_found(format!("Machine not found: {}", machine_id)));
        }
        
        // チャネルの作成（バッファサイズは調整可能）
        let (tx, rx) = mpsc::channel(128);
        
        // 監視リストに追加
        let mut watchers = self.watchers.write().unwrap();
        if let Some(channels) = watchers.get_mut(&machine_id) {
            channels.push(tx);
        } else {
            return Err(Status::internal("Watcher registration failed"));
        }
        
        // ストリームを返す
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    /// 型安全なクライアントコードの生成
    async fn generate_client_code(
        &self,
        request: Request<GenerateClientCodeRequest>,
    ) -> std::result::Result<Response<GenerateClientCodeResponse>, Status> {
        let req = request.into_inner();
        let machine_id = req.machine_id;
        let language = req.language;
        
        // マシンの存在確認
        let machines = self.machines.read().unwrap();
        let machine = machines.get(&machine_id).ok_or_else(|| {
            Status::not_found(format!("Machine not found: {}", machine_id))
        })?;
        
        // ここでは簡略化のため、実際のコード生成は省略
        // 実際には言語ごとのテンプレートエンジンなどを使用する
        
        let code = match language.as_str() {
            "rust" => {
                format!(
                    r#"
// Generated code for state machine: {}
use rustate_grpc::client::{{StateMachineServiceClient, types::*}};

pub struct {}Client {{
    client: StateMachineServiceClient<tonic::transport::Channel>,
    machine_id: String,
}}

impl {}Client {{
    pub async fn connect(addr: &str, machine_id: &str) -> Result<Self, tonic::transport::Error> {{
        let client = StateMachineServiceClient::connect(addr).await?;
        Ok(Self {{
            client,
            machine_id: machine_id.to_string(),
        }})
    }}
    
    pub async fn send_event(&mut self, event_type: &str) -> Result<SendEventResponse, tonic::Status> {{
        let event = EventDefinition {{
            machine_id: self.machine_id.clone(),
            event_type: event_type.to_string(),
            payload: "{{}}".to_string(),
        }};
        
        let request = SendEventRequest {{
            event: Some(event),
        }};
        
        Ok(self.client.send_event(request).await?)
    }}
    
    // 他のメソッドも同様に...
}}
                "#,
                    machine_id,
                    machine_id.replace("-", ""),
                    machine_id.replace("-", "")
                )
            }
            "typescript" => {
                format!(
                    r#"
// Generated code for state machine: {}
import {{ grpc }} from "@improbable-eng/grpc-web";
import {{ StateMachineService }} from "./proto/rustate_pb_service";
import {{ 
  SendEventRequest, 
  EventDefinition,
  GetStateRequest
}} from "./proto/rustate_pb";

export class {}Client {{
  private machineId: string;
  
  constructor(machineId: string) {{
    this.machineId = machineId;
  }}
  
  async sendEvent(eventType: string): Promise<any> {{
    return new Promise((resolve, reject) => {{
      const event = new EventDefinition();
      event.setMachineId(this.machineId);
      event.setEventType(eventType);
      event.setPayload("{{}}");
      
      const request = new SendEventRequest();
      request.setEvent(event);
      
      grpc.unary(StateMachineService.SendEvent, {{
        request,
        host: "https://your-server.example",
        onEnd: res => {{
          if (res.status !== grpc.Code.OK) {{
            reject(new Error(res.statusMessage));
            return;
          }}
          resolve(res.message?.toObject());
        }}
      }});
    }});
  }}
  
  // 他のメソッドも同様に...
}}
                "#,
                    machine_id,
                    machine_id.replace("-", "")
                )
            }
            _ => {
                return Err(Status::invalid_argument(format!(
                    "Unsupported language: {}",
                    language
                )));
            }
        };
        
        // レスポンス作成
        let response = GenerateClientCodeResponse { code };
        
        Ok(Response::new(response))
    }
} 