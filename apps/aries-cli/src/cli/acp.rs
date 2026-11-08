use agent_client_protocol::Stdio;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(about = "Start an Agent Communication Protocol (ACP) server")]
pub struct AcpArgs;

pub async fn execute(_args: AcpArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(gctx.root_dir());
    let setting = loader.load().await?;

    aries_logger::init(gctx.root_dir().join("logs"));

    let transport = Stdio::new();
    aries_acp::v1::run(gctx, setting, transport).await?;

    Ok(())
}
