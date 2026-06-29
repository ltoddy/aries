use agent_client_protocol::Stdio;
use aries_context::GlobalContext;
use aries_init::SettingLoader;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = SettingLoader::new(&gctx.root_dir);
    let setting = loader.load().await?;

    aries_logger::init(gctx.root_dir.join("logs"));

    aries_acp::v1::run(gctx, setting, Stdio::new()).await
}
