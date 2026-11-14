use agent_client_protocol::Stdio;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

use crate::Result;
use crate::error::Error;

#[derive(Parser, Debug, Clone)]
#[command(about = "Start an Agent Communication Protocol (ACP) server")]
pub struct AcpArgs;

pub async fn execute(_args: AcpArgs, gctx: GlobalContext) -> Result<()> {
    let loader = SettingLoader::new(gctx.root_dir());
    let setting = loader.load().await.map_err(Error::setting)?;

    aries_logger::init(gctx.root_dir().join("logs"));

    let transport = Stdio::new();
    aries_acp::v1::run(gctx, setting, transport).await.map_err(Error::acp)?;

    Ok(())
}
