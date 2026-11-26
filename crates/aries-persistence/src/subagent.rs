use jiff::Timestamp;
use toasty::Db;
use toasty::stmt::{Assign, IntoExpr};

#[derive(Debug, Clone, toasty::Model)]
#[table = "subagents"]
pub struct Subagent {
    #[key]
    #[auto(increment)]
    pub id: u64,

    #[index]
    pub session_id: String,
    #[unique]
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub last_prompt: String,
    pub dir: String,
    #[index]
    #[auto]
    pub created_at: Timestamp,
}

#[derive(Clone)]
pub struct SubagentRepository {
    db: Db,
}

impl SubagentRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn save(
        &mut self,
        session_id: impl IntoExpr<String> + Assign<String>,
        agent_id: impl IntoExpr<String>,
        dir: impl IntoExpr<String> + Assign<String>,
        name: impl IntoExpr<String> + Assign<String>,
        description: impl IntoExpr<String> + Assign<String>,
        prompt: impl IntoExpr<String> + Assign<String>,
    ) -> toasty::Result<Subagent> {
        Subagent::upsert_by_agent_id(agent_id)
            .session_id(session_id)
            .name(name)
            .description(description)
            .last_prompt(prompt)
            .dir(dir)
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_session_id(
        &mut self,
        session_id: impl IntoExpr<String>,
    ) -> toasty::Result<Vec<Subagent>> {
        Subagent::filter(Subagent::fields().session_id().eq(session_id))
            .order_by(Subagent::fields().created_at().asc())
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_agent_id(
        &mut self,
        agent_id: impl IntoExpr<String>,
    ) -> toasty::Result<Option<Subagent>> {
        Subagent::filter(Subagent::fields().agent_id().eq(agent_id))
            .first()
            .exec(&mut self.db)
            .await
    }

    pub async fn delete_by_session_id(
        &mut self,
        session_id: impl IntoExpr<String>,
    ) -> toasty::Result<()> {
        Subagent::filter(Subagent::fields().session_id().eq(session_id))
            .delete()
            .exec(&mut self.db)
            .await
    }
}
