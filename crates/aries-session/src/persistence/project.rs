use jiff::Timestamp;

#[derive(Debug, Clone, toasty::Model)]
pub struct Project {
    #[key]
    #[auto(increment)]
    pub id: u64,

    #[unique]
    pub dir: String,

    pub name: String,

    #[index]
    #[auto]
    pub created_at: Timestamp,
}

pub struct ProjectRepository {
    db: toasty::Db,
}

impl ProjectRepository {
    pub fn new(db: toasty::Db) -> Self {
        Self { db }
    }

    pub async fn upsert_by_dir(&mut self, dir: String, name: String) -> toasty::Result<Project> {
        if let Ok(value) = self.find_last_by_dir(&dir).await {
            return Ok(value);
        }
        self.create(dir, name).await
    }

    pub async fn all(&mut self) -> toasty::Result<Vec<Project>> {
        let values = Project::all()
            .order_by(Project::fields().created_at().desc())
            .exec(&mut self.db)
            .await?;
        Ok(values)
    }

    pub async fn delete_by_dir(&mut self, dir: String) -> toasty::Result<()> {
        Project::delete_by_dir(&mut self.db, &dir).await
    }

    async fn create(&mut self, dir: String, name: String) -> toasty::Result<Project> {
        Project::create().dir(dir).name(name).exec(&mut self.db).await
    }

    async fn find_last_by_dir(&mut self, dir: &str) -> toasty::Result<Project> {
        let value = Project::get_by_dir(&mut self.db, dir).await?;
        Ok(value)
    }
}
