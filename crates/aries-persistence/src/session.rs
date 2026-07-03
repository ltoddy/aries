use jiff::Timestamp;
use toasty::Db;
use toasty::codegen_support::{FieldExprTarget, List};
use toasty::stmt::{Assign, IntoExpr};

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
    pub transcript_path: String,

    #[index]
    #[auto]
    pub created_at: Timestamp,

    pub updated_at: Timestamp,
}

#[derive(Clone)]
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
        transcript_path: impl IntoExpr<String>,
    ) -> toasty::Result<Session> {
        Session::create()
            .session_id(session_id)
            .cwd(cwd)
            .root_dir(root_dir)
            .transcript_path(transcript_path)
            .updated_at(Timestamp::now())
            .exec(&mut self.db)
            .await
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

    pub async fn update_title_by_session_id(
        &mut self,
        session_id: impl IntoExpr<String>,
        title: impl Assign<FieldExprTarget<Option<String>>>,
    ) -> toasty::Result<()> {
        Session::update_by_session_id(session_id)
            .title(title)
            .updated_at(Timestamp::now())
            .exec(&mut self.db)
            .await
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
