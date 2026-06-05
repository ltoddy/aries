use agent_client_protocol::Stdio;
use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let config = loader.load_or_setup().await?;

    aries_acp::run(gctx, config, Stdio::new()).await
}
