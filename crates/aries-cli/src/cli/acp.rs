use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use agent_client_protocol::Stdio;
use aries_init::{GlobalContext, SettingLoader};
use clap::Parser;

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum AcpVersion {
    V1,
    V2,
}

#[derive(Debug, Clone)]
pub enum Transport {
    Stdio,
    Tcp(SocketAddr),
}

impl FromStr for Transport {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "stdio" {
            return Ok(Transport::Stdio);
        }

        s.parse::<SocketAddr>()
            .map(Transport::Tcp)
            .map_err(|e| format!("invalid transport `{s}`: expected `stdio` or `HOST:PORT` ({e})"))
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transport::Stdio => write!(f, "stdio"),
            Transport::Tcp(addr) => write!(f, "{addr}"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Start an Agent Communication Protocol (ACP) server")]
pub struct AcpArgs {
    #[arg(value_enum, default_value = "v1", help = "ACP protocol version (v1 or v2)")]
    pub version: AcpVersion,

    #[arg(long, help = "Run in bare mode")]
    pub bare: bool,

    #[arg(
        long,
        value_name = "TRANSPORT",
        default_value = "stdio",
        help = "Transport to serve ACP over: `stdio` or `HOST:PORT`"
    )]
    pub transport: Transport,
}

pub async fn execute(
    AcpArgs { version, bare, transport }: AcpArgs,
    gctx: GlobalContext,
) -> anyhow::Result<()> {
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    match transport {
        Transport::Stdio => {
            let transport = Stdio::new();
            match version {
                AcpVersion::V1 => aries_acp::v1::run(gctx, setting, transport, bare).await,
                AcpVersion::V2 => aries_acp::v2::run(gctx, setting, transport).await,
            }
        },
        Transport::Tcp(addr) => {
            let transport = aries_acp::transport::TcpTransport::bind(addr).await?;
            match version {
                AcpVersion::V1 => aries_acp::v1::run(gctx, setting, transport, bare).await,
                AcpVersion::V2 => aries_acp::v2::run(gctx, setting, transport).await,
            }
        },
    }
}
