use jiff::Timestamp;
use toasty::Db;
use toasty::stmt::IntoExpr;

// 用于记录节省了多少 token
#[derive(Debug, Clone, toasty::Model)]
#[table = "token_audit"]
pub struct TokenAudit {
    #[key]
    #[auto(increment)]
    id: u64,

    command: String,
    original_tokens: u64,
    optimized_tokens: u64,
    saved_tokens: u64,
    savings_percent: f64,

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
}
