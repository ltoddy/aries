use std::path::PathBuf;

use aries_extension::hook::input::SessionStartSource;
use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, ModelConfig, Setting};
use aries_persistence::SessionRepository;
use toasty::Db;

use crate::session::{Session, SessionError};

impl Session {
    pub async fn load_session(
        id: impl Into<String>,
        gctx: GlobalContext,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let id = id.into();

        let mut session_repo = SessionRepository::new(db.clone());
        let session =
            session_repo.find_last_by_session_id(&id).await.map_err(SessionError::db)?.unwrap();
        let cwd = PathBuf::from(&session.cwd);

        Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Resume,
        )
        .await
    }
}
