use std::future;
use std::time::Duration;

use aries_init::GlobalContext;
use aries_proto::aries_service_client::AriesServiceClient;
use gpui_kit::{App, Global};
use tokio::runtime::Runtime;
use tokio::time;
use tonic::transport::{Channel, Endpoint};

use crate::error;

const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("无法连接本地 IPC 服务")]
    Endpoint { source: tonic::transport::Error },
}

pub struct Ipc {
    #[expect(dead_code, reason = "holds the runtime that drives the IPC server and client")]
    runtime: Runtime,
    client: AriesServiceClient<Channel>,
}

impl Global for Ipc {}

impl Ipc {
    pub fn start(runtime: Runtime, context: &GlobalContext) -> Result<Self, IpcError> {
        let root = context.root_dir().to_path_buf();
        let endpoint = Endpoint::from_shared(format!("unix:{}", root.join("aries.sock").display()))
            .map_err(|source| IpcError::Endpoint { source })?;
        runtime.spawn(async move {
            let result = aries_ipc::serve(&root, future::pending()).await;
            if let Err(source) = result {
                error::log("本地 IPC 服务退出", &source);
            }
        });
        let client = runtime.block_on(connect(&endpoint));
        tracing::info!("Aries Desktop connected to the local IPC server");
        Ok(Self { runtime, client })
    }

    pub fn client(&self) -> AriesServiceClient<Channel> {
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
