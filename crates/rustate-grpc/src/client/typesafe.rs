use std::fmt;
use tonic::transport::Channel;
use tonic::Status;

use crate::proto::state_machine_service_client::StateMachineServiceClient;
use crate::proto::{
    self, BatchEventsRequest, BatchEventsResponse, CreateMachineRequest, CreateMachineResponse,
    EventDefinition, GetStateRequest, MachineState, SendEventRequest, SendEventResponse,
    StateChangeEvent, UnregisterMachineRequest, UnregisterMachineResponse, WatchMachineRequest,
};

/// 型安全なステートマシンクライアント
pub struct TypeSafeClient<TEvent, TContext = ()>
where
    TEvent: fmt::Display + Clone,
    TContext: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    client: StateMachineServiceClient<Channel>,
    machine_id: String,
    _event_type: std::marker::PhantomData<TEvent>,
    _context_type: std::marker::PhantomData<TContext>,
}

impl<TEvent, TContext> TypeSafeClient<TEvent, TContext>
where
    TEvent: fmt::Display + Clone,
    TContext: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    /// 新しいクライアントを作成
    pub async fn new(addr: &str, machine_id: &str) -> Result<Self, tonic::transport::Error> {
        let client = StateMachineServiceClient::connect(addr.to_string()).await?;

        Ok(Self {
            client,
            machine_id: machine_id.to_string(),
            _event_type: std::marker::PhantomData,
            _context_type: std::marker::PhantomData,
        })
    }

    /// 既存のクライアントから作成
    pub fn from_client(client: StateMachineServiceClient<Channel>, machine_id: &str) -> Self {
        Self {
            client,
            machine_id: machine_id.to_string(),
            _event_type: std::marker::PhantomData,
            _context_type: std::marker::PhantomData,
        }
    }

    /// イベントを送信
    pub async fn send_event(&mut self, event: TEvent) -> Result<EventResult<TContext>, Status> {
        let event_def = EventDefinition {
            machine_id: self.machine_id.clone(),
            event_type: event.to_string(),
            payload: "{}".to_string(), // 簡略化
        };

        let request = SendEventRequest {
            event: Some(event_def),
        };

        let response = self.client.send_event(request).await?;

        // レスポンスをEventResultに変換
        if let Some(result) = response.get_ref().result.as_ref() {
            let context = if !result.context.is_empty() {
                serde_json::from_str(&result.context).unwrap_or_default()
            } else {
                TContext::default()
            };

            Ok(EventResult {
                success: result.success,
                states_changed: result.states_changed.clone(),
                context,
                error_message: result.error_message.clone(),
            })
        } else {
            Ok(EventResult {
                success: false,
                states_changed: vec![],
                context: TContext::default(),
                error_message: "No result returned".to_string(),
            })
        }
    }

    /// 現在の状態を取得
    pub async fn get_state(&mut self) -> Result<MachineState<TContext>, Status> {
        let request = GetStateRequest {
            machine_id: self.machine_id.clone(),
        };

        let response = self.client.get_state(request).await?;
        let proto_state = response.get_ref();

        // レスポンスをMachineStateに変換
        let context = if !proto_state.context.is_empty() {
            serde_json::from_str(&proto_state.context).unwrap_or_default()
        } else {
            TContext::default()
        };

        Ok(MachineState {
            machine_id: proto_state.machine_id.clone(),
            current_states: proto_state.current_states.clone(),
            context,
        })
    }

    /// 複数イベントをバッチ処理
    pub async fn batch_events(
        &mut self,
        events: Vec<TEvent>,
    ) -> Result<BatchEventsResult<TContext>, Status> {
        let proto_events = events
            .into_iter()
            .map(|evt| EventDefinition {
                machine_id: self.machine_id.clone(),
                event_type: evt.to_string(),
                payload: "{}".to_string(), // 簡略化
            })
            .collect();

        let request = BatchEventsRequest {
            machine_id: self.machine_id.clone(),
            events: proto_events,
        };

        let response = self.client.batch_events(request).await?;
        let proto_response = response.get_ref();

        // レスポンスをBatchEventsResultに変換
        let results = proto_response
            .results
            .iter()
            .map(|res| {
                let context = if !res.context.is_empty() {
                    serde_json::from_str(&res.context).unwrap_or_default()
                } else {
                    TContext::default()
                };

                EventResult {
                    success: res.success,
                    states_changed: res.states_changed.clone(),
                    context,
                    error_message: res.error_message.clone(),
                }
            })
            .collect();

        let final_state = if let Some(state) = &proto_response.final_state {
            let context = if !state.context.is_empty() {
                serde_json::from_str(&state.context).unwrap_or_default()
            } else {
                TContext::default()
            };

            Some(MachineState {
                machine_id: state.machine_id.clone(),
                current_states: state.current_states.clone(),
                context,
            })
        } else {
            None
        };

        Ok(BatchEventsResult {
            results,
            final_state,
        })
    }

    /// ステートマシンの登録解除
    pub async fn unregister(&mut self) -> Result<UnregisterResult, Status> {
        let request = UnregisterMachineRequest {
            machine_id: self.machine_id.clone(),
        };

        let response = self.client.unregister_machine(request).await?;
        let proto_response = response.get_ref();

        Ok(UnregisterResult {
            success: proto_response.success,
            message: proto_response.message.clone(),
        })
    }

    /// ステートマシンの状態変化を監視
    pub async fn watch(
        &mut self,
        include_context: bool,
    ) -> Result<tonic::Response<tonic::Streaming<StateChangeEvent>>, Status> {
        let request = WatchMachineRequest {
            machine_id: self.machine_id.clone(),
            include_context,
        };

        self.client.watch_machine(request).await
    }
}

/// イベント処理結果
#[derive(Debug, Clone)]
pub struct EventResult<TContext> {
    pub success: bool,
    pub states_changed: Vec<String>,
    pub context: TContext,
    pub error_message: String,
}

/// ステートマシンの状態
#[derive(Debug, Clone)]
pub struct MachineState<TContext> {
    pub machine_id: String,
    pub current_states: Vec<String>,
    pub context: TContext,
}

/// バッチイベント処理結果
#[derive(Debug, Clone)]
pub struct BatchEventsResult<TContext> {
    pub results: Vec<EventResult<TContext>>,
    pub final_state: Option<MachineState<TContext>>,
}

/// ステートマシン登録解除結果
#[derive(Debug, Clone)]
pub struct UnregisterResult {
    pub success: bool,
    pub message: String,
}
