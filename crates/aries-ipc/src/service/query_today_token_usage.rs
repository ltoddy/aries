use aries_proto::{QueryTodayTokenUsageRequest, QueryTodayTokenUsageResponse, TokenUsage};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use tonic::{Request, Response, Status};

use crate::service::{AriesServiceImpl, accumulate};

impl AriesServiceImpl {
    pub async fn __query_today_token_usage(
        &self,
        _request: Request<QueryTodayTokenUsageRequest>,
    ) -> Result<Response<QueryTodayTokenUsageResponse>, Status> {
        let timezone = TimeZone::system();
        let now = Timestamp::now().to_zoned(timezone);

        let low = now
            .start_of_day()
            .map_err(|err| Status::internal(format!("failed to calculate start of day: {err}")))?
            .timestamp();
        let high = now.timestamp();

        let mut repository = self.token_usage_repo.clone();
        let records = repository
            .find_by_created_at_between(low, high)
            .await
            .map_err(|err| Status::internal(format!("failed to query token usage: {err}")))?;

        let mut usage = TokenUsage::default();
        for record in &records {
            accumulate(&mut usage, record);
        }
        let resp = QueryTodayTokenUsageResponse { token_usage: Some(usage) };
        Ok(Response::new(resp))
    }
}
