use jiff::Timestamp;
use toasty::Db;
use toasty::codegen_support::List;
use toasty::stmt::IntoExpr;

#[derive(Debug, Clone, toasty::Model)]
#[table = "token_usage"]
pub struct TokenUsage {
    #[key]
    #[auto(increment)]
    pub id: u64,

    #[index]
    pub cwd: String,

    #[index]
    #[auto]
    pub created_at: Timestamp,

    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,

    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_write_cost: f64,
    pub total_cost: f64,
}

#[derive(Clone)]
pub struct TokenUsageRepository {
    db: Db,
}

impl TokenUsageRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &mut self,
        cwd: impl IntoExpr<String>,
        input_tokens: u64,
        output_tokens: u64,
        total_tokens: u64,
        cached_input_tokens: u64,
        reasoning_tokens: u64,
        input_cost: f64,
        output_cost: f64,
        cache_read_cost: f64,
        cache_write_cost: f64,
        total_cost: f64,
    ) -> toasty::Result<TokenUsage> {
        TokenUsage::create()
            .cwd(cwd)
            .input_tokens(input_tokens)
            .output_tokens(output_tokens)
            .total_tokens(total_tokens)
            .cached_input_tokens(cached_input_tokens)
            .reasoning_tokens(reasoning_tokens)
            .input_cost(input_cost)
            .output_cost(output_cost)
            .cache_read_cost(cache_read_cost)
            .cache_write_cost(cache_write_cost)
            .total_cost(total_cost)
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_cwd(
        &mut self,
        cwd: impl IntoExpr<String>,
    ) -> toasty::Result<Vec<TokenUsage>> {
        TokenUsage::filter(TokenUsage::fields().cwd().eq(cwd)).exec(&mut self.db).await
    }

    pub async fn find_by_created_at_less_than(
        &mut self,
        created_at: Timestamp,
    ) -> toasty::Result<Vec<TokenUsage>> {
        TokenUsage::filter(TokenUsage::fields().created_at().lt(created_at))
            .exec(&mut self.db)
            .await
    }

    pub async fn find_by_created_at_between(
        &mut self,
        low: Timestamp,
        high: Timestamp,
    ) -> toasty::Result<Vec<TokenUsage>> {
        TokenUsage::filter(
            TokenUsage::fields()
                .created_at()
                .ge(low)
                .and(TokenUsage::fields().created_at().le(high)),
        )
        .exec(&mut self.db)
        .await
    }

    pub async fn delete_by_id(&mut self, id: u64) -> toasty::Result<()> {
        TokenUsage::delete_by_id(&mut self.db, id).await
    }

    pub async fn delete_by_id_in(&mut self, ids: impl IntoExpr<List<u64>>) -> toasty::Result<()> {
        TokenUsage::filter(TokenUsage::fields().id().in_list(ids)).delete().exec(&mut self.db).await
    }
}
