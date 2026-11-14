use aries_init::GlobalContext;

use crate::Result;

pub async fn execute(gctx: GlobalContext) -> Result<()> {
    aries_init::gc(gctx.root_dir()).await;
    Ok(())
}
