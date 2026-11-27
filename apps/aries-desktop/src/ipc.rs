use std::future;
use std::time::Duration;

use aries_init::GlobalContext;
use aries_proto::aries_service_client::AriesServiceClient;
use aries_proto::{
    GetTodayTokenUsageRequest, GetTodayTokenUsageResponse, ListTokenUsagesRequest,
    ListTokenUsagesResponse,
};
use gpui_kit::{App, Global};
use tokio::runtime::{Handle, Runtime};
use tokio::time;
use tonic::transport::{Channel, Endpoint};

use crate::error;

const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("无法连接本地 IPC 服务")]
    Endpoint { source: tonic::transport::Error },
    #[error("无法查询本地 IPC 服务的用量数据")]
    ListTokenUsages { source: tonic::Status },
    #[error("无法查询本地 IPC 服务的当日用量数据")]
    GetTodayTokenUsage { source: tonic::Status },
}

impl IpcError {
    fn endpoint(source: tonic::transport::Error) -> Self {
        Self::Endpoint { source }
    }

    fn list_token_usages(source: tonic::Status) -> Self {
        Self::ListTokenUsages { source }
    }

    fn get_today_token_usage(source: tonic::Status) -> Self {
        Self::GetTodayTokenUsage { source }
    }
}

#[derive(Clone)]
pub struct Client {
    runtime: Handle,
    service: AriesServiceClient<Channel>,
}

impl Client {
    fn new(runtime: Handle, service: AriesServiceClient<Channel>) -> Self {
        Self { runtime, service }
    }

    pub fn list_token_usages(&self) -> Result<ListTokenUsagesResponse, IpcError> {
        let mut service = self.service.clone();
        self.runtime
            .block_on(service.list_token_usages(ListTokenUsagesRequest {}))
            .map(tonic::Response::into_inner)
            .map_err(IpcError::list_token_usages)
    }

    pub fn get_today_token_usage(&self) -> Result<GetTodayTokenUsageResponse, IpcError> {
        let mut service = self.service.clone();
        self.runtime
            .block_on(service.get_today_token_usage(GetTodayTokenUsageRequest {}))
            .map(tonic::Response::into_inner)
            .map_err(IpcError::get_today_token_usage)
    }
}

pub struct Ipc {
    #[expect(dead_code, reason = "keeps the runtime that drives the IPC server and client alive")]
    runtime: Runtime,
    client: Client,
}

impl Global for Ipc {}

impl Ipc {
    pub fn start(runtime: Runtime, context: &GlobalContext) -> Result<Self, IpcError> {
        let root = context.root_dir().to_path_buf();
        let endpoint = Endpoint::from_shared(format!("unix:{}", root.join("aries.sock").display()))
            .map_err(IpcError::endpoint)?;
        runtime.spawn(async move {
            let result = aries_ipc::serve(&root, future::pending()).await;
            if let Err(source) = result {
                error::log("本地 IPC 服务退出", &source);
            }
        });
        let service = runtime.block_on(connect(&endpoint));
        let client = Client::new(runtime.handle().clone(), service);
        tracing::info!("Aries Desktop connected to the local IPC server");
        Ok(Self { runtime, client })
    }

    pub fn client(&self) -> Client {
        self.client.clone()
    }

    pub fn init(self, cx: &mut App) {
        cx.set_global(self);
    }
}

async fn connect(endpoint: &Endpoint) -> AriesServiceClient<Channel> {
    loop {
        if let Ok(channel) = endpoint.connect().await {
            return AriesServiceClient::new(channel);
        }
        time::sleep(CONNECT_RETRY_DELAY).await;
    }
}
