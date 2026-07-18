use agent_client_protocol::Stdio;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum AcpVersion {
    V1,
    V2,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Start an Agent Communication Protocol (ACP) server")]
pub struct AcpArgs {
    #[arg(value_enum, default_value = "v1", help = "ACP protocol version (v1 or v2)")]
    pub version: AcpVersion,
}

pub async fn execute(args: AcpArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    match args.version {
        AcpVersion::V1 => aries_acp::v1::run(gctx, setting, Stdio::new()).await,
        AcpVersion::V2 => aries_acp::v2::run(gctx, setting, Stdio::new()).await,
    }
}
