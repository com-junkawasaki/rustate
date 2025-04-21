use tonic::transport::{Channel, Endpoint};
use std::time::Duration;

/// トランスポート設定
pub struct TransportConfig {
    pub connect_timeout: Option<Duration>,
    pub timeout: Option<Duration>,
    pub concurrency_limit: Option<usize>,
    pub rate_limit: Option<(u64, Duration)>,
    pub user_agent: Option<String>,
    pub keep_alive: Option<Duration>,
    pub tcp_nodelay: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Some(Duration::from_secs(10)),
            timeout: Some(Duration::from_secs(30)),
            concurrency_limit: Some(1024),
            rate_limit: None,
            user_agent: Some("rustate-grpc-client/0.1.0".to_string()),
            keep_alive: Some(Duration::from_secs(60)),
            tcp_nodelay: true,
        }
    }
}

/// gRPC接続の設定管理
pub struct ClientTransport {
    pub(crate) endpoint: Endpoint,
}

impl ClientTransport {
    /// 新しいトランスポートを作成
    pub fn new(addr: &str) -> Result<Self, tonic::transport::Error> {
        let endpoint = Endpoint::from_shared(addr.to_string())?;
        Ok(Self { endpoint })
    }
    
    /// デフォルト設定で構成
    pub fn with_default_config(mut self) -> Self {
        let config = TransportConfig::default();
        self.apply_config(&config);
        self
    }
    
    /// カスタム設定で構成
    pub fn with_config(mut self, config: &TransportConfig) -> Self {
        self.apply_config(config);
        self
    }
    
    /// 設定を適用
    fn apply_config(&mut self, config: &TransportConfig) {
        self.endpoint = if let Some(timeout) = config.connect_timeout {
            self.endpoint.clone().connect_timeout(timeout)
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = if let Some(timeout) = config.timeout {
            self.endpoint.clone().timeout(timeout)
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = if let Some(limit) = config.concurrency_limit {
            self.endpoint.clone().concurrency_limit(limit)
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = if let Some((limit, duration)) = config.rate_limit {
            self.endpoint.clone().rate_limit(limit, duration)
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = if let Some(agent) = &config.user_agent {
            self.endpoint.clone().user_agent(agent.clone())
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = if let Some(duration) = config.keep_alive {
            self.endpoint.clone().keep_alive_timeout(duration)
        } else {
            self.endpoint.clone()
        };
        
        self.endpoint = self.endpoint.clone().tcp_nodelay(config.tcp_nodelay);
    }
    
    /// チャネルを作成
    pub async fn connect(&self) -> Result<Channel, tonic::transport::Error> {
        self.endpoint.connect().await
    }
} 