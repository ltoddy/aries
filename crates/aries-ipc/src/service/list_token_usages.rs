use std::collections::{BTreeMap, BTreeSet};

use aries_proto::{DailyUsage, ListTokenUsagesRequest, ListTokenUsagesResponse, TokenUsage};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use tonic::{Request, Response, Status};

use crate::service::{AriesServiceImpl, accumulate, date_to_proto};

impl AriesServiceImpl {
    pub async fn __list_token_usages(
        &self,
        _request: Request<ListTokenUsagesRequest>,
    ) -> Result<Response<ListTokenUsagesResponse>, Status> {
        let timezone = TimeZone::system();
        let now = Timestamp::now().to_zoned(timezone.clone());

        let low = now
            .checked_sub(Span::new().years(1))
            .map_err(|err| Status::internal(format!("failed to calculate past year: {err}")))?
            .timestamp();
        let high = now.timestamp();

        let mut repository = self.token_usage_repo.clone();
        let records = repository
            .find_by_created_at_between(low, high)
            .await
            .map_err(|err| Status::internal(format!("failed to query token usage: {err}")))?;

        let projects = records
            .iter()
            .map(|r| r.cwd.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let daily =
            records.iter().fold(BTreeMap::<Date, TokenUsage>::new(), |mut daily, record| {
                let date = record.created_at.to_zoned(timezone.clone()).date();

                accumulate(daily.entry(date).or_default(), record);
                daily
            });
        let daily_usages = daily
            .into_iter()
            .map(|(date, usage)| DailyUsage {
                date: Some(date_to_proto(date)),
                token_usage: Some(usage),
            })
            .collect();
        let resp = ListTokenUsagesResponse { daily_usages, projects };
        Ok(Response::new(resp))
    }
}
