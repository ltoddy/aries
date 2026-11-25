use std::future;
use std::time::Duration;

use aries_init::GlobalContext;
use aries_proto::aries_service_client::AriesServiceClient;
use aries_proto::{QueryTokenUsagesRequest, QueryTokenUsagesResponse};
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
    QueryTokenUsages { source: tonic::Status },
}

impl IpcError {
    fn endpoint(source: tonic::transport::Error) -> Self {
        Self::Endpoint { source }
    }

    fn query_token_usages(source: tonic::Status) -> Self {
        Self::QueryTokenUsages { source }
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

    pub fn query_token_usages(&self) -> Result<QueryTokenUsagesResponse, IpcError> {
        let mut service = self.service.clone();
        self.runtime
            .block_on(service.query_token_usages(QueryTokenUsagesRequest {}))
            .map(tonic::Response::into_inner)
            .map_err(IpcError::query_token_usages)
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
