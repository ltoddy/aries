use jiff::Timestamp;
use toasty::Db;
use toasty::codegen_support::List;
use toasty::stmt::IntoExpr;

// 用于记录节省了多少 token
#[derive(Debug, Clone, toasty::Model)]
#[table = "token_audit"]
pub struct TokenAudit {
    #[key]
    #[auto(increment)]
    pub id: u64,

    pub command: String,
    pub original_tokens: u64,
    pub optimized_tokens: u64,
    pub saved_tokens: u64,
    pub savings_percent: f64,

    #[index]
    #[auto]
    pub created_at: Timestamp,
}

pub struct TokenAuditRepository {
    db: Db,
}

impl TokenAuditRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(
        &mut self,
        command: impl IntoExpr<String>,
        original_tokens: u64,
        optimized_tokens: u64,
    ) -> toasty::Result<TokenAudit> {
        let saved_tokens = original_tokens.saturating_sub(optimized_tokens);
        let savings_percent = (saved_tokens as f64 / original_tokens as f64) * 100.0;

        TokenAudit::create()
            .command(command)
            .original_tokens(original_tokens)
            .optimized_tokens(optimized_tokens)
            .saved_tokens(saved_tokens)
            .savings_percent(savings_percent)
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_created_at_less_than(
        &mut self,
        created_at: Timestamp,
    ) -> toasty::Result<Vec<TokenAudit>> {
        TokenAudit::filter(TokenAudit::fields().created_at().lt(created_at))
            .exec(&mut self.db)
            .await
    }

    pub async fn delete_by_id_in(&mut self, ids: impl IntoExpr<List<u64>>) -> toasty::Result<()> {
        TokenAudit::filter(TokenAudit::fields().id().in_list(ids)).delete().exec(&mut self.db).await
    }
}
