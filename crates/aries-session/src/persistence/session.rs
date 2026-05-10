use itertools::Itertools;
use jiff::Timestamp;
use toasty::Db;
use toasty::codegen_support::List;
use toasty::stmt::IntoExpr;

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
    pub cwd: String,

    pub root_dir: String,

    #[index]
    #[auto]
    pub created_at: Timestamp,

    pub updated_at: Timestamp,
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
        session_id: impl IntoExpr<String>,
        cwd: impl IntoExpr<String>,
        root_dir: impl IntoExpr<String>,
    ) -> toasty::Result<Session> {
        Session::create()
            .session_id(session_id)
            .cwd(cwd)
            .root_dir(root_dir)
            .updated_at(Timestamp::now())
            .exec(&mut self.db)
            .await
    }

    pub async fn find_projects(&mut self) -> toasty::Result<Vec<String>> {
        // TODO: use sql distinct statemate

        let values = Session::all()
            .order_by(Session::fields().created_at().desc())
            .exec(&mut self.db)
            .await?
            .into_iter()
            .map(|v| v.cwd)
            .unique()
            .collect::<Vec<_>>();
        Ok(values)
    }

    pub async fn find_by_cwd(
        &mut self,
        cwd: impl IntoExpr<String>,
    ) -> toasty::Result<Vec<Session>> {
        Session::filter(Session::fields().cwd().eq(cwd))
            .order_by(Session::fields().created_at().desc())
            .exec(&mut self.db)
            .await
    }

    pub async fn find(&mut self) -> toasty::Result<Vec<Session>> {
        Session::all().order_by(Session::fields().updated_at().desc()).exec(&mut self.db).await
    }

    pub async fn find_last_by_session_id(
        &mut self,
        session_id: impl IntoExpr<String>,
    ) -> toasty::Result<Session> {
        Session::filter(Session::fields().session_id().eq(session_id))
            .one()
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_session_id_in(
        &mut self,
        session_ids: impl IntoExpr<List<String>>,
    ) -> toasty::Result<Vec<Session>> {
        Session::filter(Session::fields().session_id().in_list(session_ids))
            .exec(&mut self.db)
            .await
    }

    pub async fn update_title_by_id(
        &mut self,
        id: impl IntoExpr<u64>,
        title: String,
    ) -> toasty::Result<()> {
        Session::update_by_id(id).title(title).updated_at(Timestamp::now()).exec(&mut self.db).await
    }

    pub async fn delete_by_session_id(
        &mut self,
        session_id: impl IntoExpr<String>,
    ) -> toasty::Result<()> {
        Session::delete_by_session_id(&mut self.db, session_id).await
    }

    pub async fn delete_by_session_id_in(
        &mut self,
        session_ids: impl IntoExpr<List<String>>,
    ) -> toasty::Result<()> {
        Session::filter(Session::fields().session_id().in_list(session_ids))
            .delete()
            .exec(&mut self.db)
            .await
    }
}
