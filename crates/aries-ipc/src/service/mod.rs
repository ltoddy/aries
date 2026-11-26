mod query_today_token_usage;
mod query_token_usages;

use aries_persistence::{TokenUsage as TokenUsageRecord, TokenUsageRepository};
use aries_proto::aries_service_server::AriesService;
use aries_proto::{
    QueryTodayTokenUsageRequest, QueryTodayTokenUsageResponse, QueryTokenUsagesRequest,
    QueryTokenUsagesResponse, TokenUsage,
};
use jiff::civil::Date;
use toasty::Db;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct AriesServiceImpl {
    token_usage_repo: TokenUsageRepository,
}

impl AriesServiceImpl {
    pub fn new(db: Db) -> Self {
        let token_usage_repo = TokenUsageRepository::new(db);

        Self { token_usage_repo }
    }
}

#[tonic::async_trait]
impl AriesService for AriesServiceImpl {
    async fn query_token_usages(
        &self,
        _req: Request<QueryTokenUsagesRequest>,
    ) -> Result<Response<QueryTokenUsagesResponse>, Status> {
        self.__query_token_usages(_req).await
    }

    async fn query_today_token_usage(
        &self,
        _req: Request<QueryTodayTokenUsageRequest>,
    ) -> Result<Response<QueryTodayTokenUsageResponse>, Status> {
        self.__query_today_token_usage(_req).await
    }
}

fn accumulate(usage: &mut TokenUsage, record: &TokenUsageRecord) {
    usage.input_tokens = usage.input_tokens.saturating_add(record.input_tokens);
    usage.output_tokens = usage.output_tokens.saturating_add(record.output_tokens);
    usage.total_tokens = usage.total_tokens.saturating_add(record.total_tokens);
    usage.cached_input_tokens =
        usage.cached_input_tokens.saturating_add(record.cached_input_tokens);
    usage.reasoning_tokens = usage.reasoning_tokens.saturating_add(record.reasoning_tokens);
}

fn date_to_proto(date: Date) -> aries_proto::Date {
    aries_proto::Date {
        year: i32::from(date.year()),
        month: date.month() as u32,
        day: date.day() as u32,
    }
}
