mod get_today_token_usage;
mod list_session_subagents;
mod list_token_usages;

use aries_persistence::{SubagentRepository, TokenUsage as TokenUsageRecord, TokenUsageRepository};
use aries_proto::aries_service_server::AriesService;
use aries_proto::{
    GetTodayTokenUsageRequest, GetTodayTokenUsageResponse, ListSessionSubagentsRequest,
    ListSessionSubagentsResponse, ListTokenUsagesRequest, ListTokenUsagesResponse, TokenUsage,
};
use jiff::civil::Date;
use toasty::Db;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct AriesServiceImpl {
    token_usage_repo: TokenUsageRepository,
    subagent_repo: SubagentRepository,
}

impl AriesServiceImpl {
    pub fn new(db: Db) -> Self {
        let token_usage_repo = TokenUsageRepository::new(db.clone());
        let subagent_repo = SubagentRepository::new(db);

        Self { token_usage_repo, subagent_repo }
    }
}

#[tonic::async_trait]
impl AriesService for AriesServiceImpl {
    async fn list_token_usages(
        &self,
        request: Request<ListTokenUsagesRequest>,
    ) -> Result<Response<ListTokenUsagesResponse>, Status> {
        self.__list_token_usages(request).await
    }

    async fn get_today_token_usage(
        &self,
        request: Request<GetTodayTokenUsageRequest>,
    ) -> Result<Response<GetTodayTokenUsageResponse>, Status> {
        self.__get_today_token_usage(request).await
    }

    async fn list_session_subagents(
        &self,
        request: Request<ListSessionSubagentsRequest>,
    ) -> Result<Response<ListSessionSubagentsResponse>, Status> {
        self.__list_session_subagents(request).await
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
