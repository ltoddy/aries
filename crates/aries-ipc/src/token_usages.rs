use std::collections::{BTreeMap, BTreeSet};

use aries_persistence::TokenUsageRepository;
use aries_proto::usage_service_server::UsageService;
use aries_proto::{DailyUsage, QueryTokenUsagesRequest, QueryTokenUsagesResponse, TokenUsage};
use jiff::Span;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use toasty::Db;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct TokenUsages {
    repository: TokenUsageRepository,
}

impl TokenUsages {
    pub fn new(db: Db) -> Self {
        Self { repository: TokenUsageRepository::new(db) }
    }
}

#[tonic::async_trait]
impl UsageService for TokenUsages {
    async fn query_token_usages(
        &self,
        request: Request<QueryTokenUsagesRequest>,
    ) -> Result<Response<QueryTokenUsagesResponse>, Status> {
        let request = request.into_inner();
        let range = request.range.ok_or_else(|| Status::invalid_argument("range is required"))?;
        let start = parse_date(range.start_inclusive, "start_inclusive")?;
        let end = parse_date(range.end_exclusive, "end_exclusive")?;
        if start >= end {
            return Err(Status::invalid_argument("start_inclusive must precede end_exclusive"));
        }

        let timezone = TimeZone::system();
        let low = start
            .to_zoned(timezone.clone())
            .map_err(|error| Status::invalid_argument(format!("invalid start_inclusive: {error}")))?
            .timestamp();
        let high = end
            .to_zoned(timezone.clone())
            .map_err(|error| Status::invalid_argument(format!("invalid end_exclusive: {error}")))?
            .timestamp()
            .checked_sub(Span::new().nanoseconds(1))
            .map_err(|error| Status::invalid_argument(format!("invalid end_exclusive: {error}")))?;

        let mut repository = self.repository.clone();
        let records = repository.find_by_created_at_between(low, high).await.map_err(|error| {
            tracing::error!(%error, "failed to query token usage");
            Status::internal("failed to query token usage")
        })?;

        let mut projects = BTreeSet::new();
        let mut daily: BTreeMap<Date, TokenUsage> = BTreeMap::new();
        for record in records {
            projects.insert(record.cwd.clone());
            if request.cwd.as_ref().is_some_and(|cwd| cwd != &record.cwd) {
                continue;
            }
            let date = record.created_at.to_zoned(timezone.clone()).date();
            let usage = daily.entry(date).or_default();
            usage.input_tokens = usage.input_tokens.saturating_add(record.input_tokens);
            usage.output_tokens = usage.output_tokens.saturating_add(record.output_tokens);
            usage.total_tokens = usage.total_tokens.saturating_add(record.total_tokens);
            usage.cached_input_tokens =
                usage.cached_input_tokens.saturating_add(record.cached_input_tokens);
            usage.reasoning_tokens = usage.reasoning_tokens.saturating_add(record.reasoning_tokens);
            usage.record_count = usage.record_count.saturating_add(1);
        }

        Ok(Response::new(query_response(daily, projects)))
    }
}

fn query_response(
    daily: BTreeMap<Date, TokenUsage>,
    projects: BTreeSet<String>,
) -> QueryTokenUsagesResponse {
    let mut days = Vec::with_capacity(daily.len());
    for (date, usage) in daily {
        days.push(daily_usage(date, usage));
    }
    QueryTokenUsagesResponse { daily: days, projects: projects.into_iter().collect() }
}

fn daily_usage(date: Date, usage: TokenUsage) -> DailyUsage {
    DailyUsage { date: Some(proto_date(date)), token_usage: Some(usage) }
}

fn proto_date(date: Date) -> aries_proto::Date {
    aries_proto::Date {
        year: i32::from(date.year()),
        month: date.month() as u32,
        day: date.day() as u32,
    }
}

fn parse_date(date: Option<aries_proto::Date>, field: &str) -> Result<Date, Status> {
    let date = date.ok_or_else(|| Status::invalid_argument(format!("{field} is required")))?;
    let year = i16::try_from(date.year)
        .map_err(|_| Status::invalid_argument(format!("invalid {field} year")))?;
    let month = i8::try_from(date.month)
        .map_err(|_| Status::invalid_argument(format!("invalid {field} month")))?;
    let day = i8::try_from(date.day)
        .map_err(|_| Status::invalid_argument(format!("invalid {field} day")))?;
    Date::new(year, month, day)
        .map_err(|error| Status::invalid_argument(format!("invalid {field}: {error}")))
}
