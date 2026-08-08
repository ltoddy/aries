use aries_init::GlobalContext;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    aries_init::gc(&gctx.root_dir).await;
    Ok(())
}
