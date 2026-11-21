use aries_init::GlobalContext;

use crate::Result;
use crate::error::Error;

pub async fn execute(gctx: GlobalContext) -> Result<()> {
    let root_dir = gctx.root_dir();
    let db = aries_persistence::connect(root_dir).await.map_err(|err| {
        Error::database("failed to connect database for garbage collection", root_dir, err)
    })?;

    aries_persistence::gc(db).await;

    Ok(())
}
