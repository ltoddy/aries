use std::path::PathBuf;

use aries_extension::hook::input::SessionStartSource;
use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, ModelConfig, Setting};
use toasty::Db;

use crate::session::{Session, SessionError};

impl Session {
    pub async fn new_session(
        gctx: GlobalContext,
        cwd: impl Into<PathBuf>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let id = nanoid::nanoid!();
        let cwd: PathBuf = cwd.into();

        let mut session = Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Startup,
        )
        .await?;

        session
            .session_repo
            .create(
                &session.state.session_id,
                session.state.cwd.display().to_string(),
                session.state.session_dir.display().to_string(),
                session.state.transcript_dir.display().to_string(),
            )
            .await
            .map_err(SessionError::db)?;

        Ok(session)
    }
}
