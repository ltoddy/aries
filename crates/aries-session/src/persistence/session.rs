use itertools::Itertools;
use jiff::Timestamp;
use toasty::Db;

#[derive(Debug, Clone, toasty::Model)]
#[table = "sessions"]
pub struct Session {
    #[key]
    #[auto(increment)]
    id: u64,

    #[index]
    pub session_id: String,

    pub title: Option<String>,

    #[index]
    pub project_dir: String,

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

    pub async fn create(&mut self, session_id: &str, project_dir: &str) -> toasty::Result<Session> {
        Session::create().session_id(session_id).project_dir(project_dir).exec(&mut self.db).await
    }

    pub async fn find_projects(&mut self) -> toasty::Result<Vec<String>> {
        // TODO: use sql distinct statemate

        let values = Session::all()
            .order_by(Session::fields().created_at().desc())
            .exec(&mut self.db)
            .await?
            .into_iter()
            .map(|v| v.project_dir)
            .unique()
            .collect::<Vec<_>>();
        Ok(values)
    }

    pub async fn find_by_project_dir(&mut self, project_dir: &str) -> toasty::Result<Vec<Session>> {
        Session::filter(Session::fields().project_dir().eq(project_dir))
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

    pub async fn update_title_by_id(&mut self, id: u64, title: &str) -> toasty::Result<()> {
        Session::update_by_id(id).title(title).exec(&mut self.db).await
    }

    pub async fn delete_by_session_id(&mut self, session_id: &str) -> toasty::Result<()> {
        Session::delete_by_session_id(&mut self.db, session_id).await
    }

    pub async fn delete_by_session_id_in(&mut self, session_ids: &[&str]) -> toasty::Result<()> {
        Session::filter(Session::fields().session_id().in_list(session_ids))
            .delete()
            .exec(&mut self.db)
            .await
    }
}
