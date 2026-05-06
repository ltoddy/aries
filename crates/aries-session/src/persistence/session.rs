use jiff::Timestamp;
use toasty::Db;

#[derive(Debug, Clone, toasty::Model)]
pub struct Session {
    #[key]
    #[auto(increment)]
    pub id: u64,

    #[index]
    pub session_id: String,

    pub title: String,

    pub root_dir: String,

    #[index]
    pub project_id: u64,

    #[index]
    #[auto]
    pub created_at: Timestamp,
}

pub struct SessionRepository {
    db: Db,
}

impl SessionRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(
        &mut self,
        session_id: &str,
        title: &str,
        root_dir: &str,
        project_id: u64,
    ) -> toasty::Result<Session> {
        Session::create()
            .session_id(session_id)
            .title(title)
            .root_dir(root_dir)
            .project_id(project_id)
            .exec(&mut self.db)
            .await
    }

    pub async fn update_title_by_id(&mut self, id: u64, title: &str) -> toasty::Result<()> {
        Session::update_by_id(id).title(title).exec(&mut self.db).await
    }

    pub async fn find_by_project_id(&mut self, project_id: u64) -> toasty::Result<Vec<Session>> {
        Session::filter(Session::fields().project_id().eq(project_id))
            .order_by(Session::fields().created_at().desc())
            .exec(&mut self.db)
            .await
    }

    pub async fn find_last_by_session_id(&mut self, session_id: &str) -> toasty::Result<Session> {
        Session::filter(Session::fields().session_id().eq(session_id))
            .order_by(Session::fields().created_at().desc())
            .one()
            .exec(&mut self.db)
            .await
    }

    pub async fn delete_by_id(&mut self, id: u64) -> toasty::Result<()> {
        Session::delete_by_id(&mut self.db, id).await
    }
}
