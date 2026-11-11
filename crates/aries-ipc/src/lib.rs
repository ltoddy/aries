use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io;
use std::path::Path;

use aries_persistence::TokenUsageRepository;
use aries_proto::aries_service_server::{AriesService, AriesServiceServer};
use aries_proto::{DailyUsage, QueryTokenUsagesRequest, QueryTokenUsagesResponse, TokenUsage};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use thiserror::Error;
use toasty::Db;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("failed to connect database: {0}")]
    Database(#[from] toasty::Error),
    #[error("failed to bind Unix socket: {0}")]
    Bind(#[from] io::Error),
    #[error("gRPC server failed: {0}")]
    Transport(#[from] tonic::transport::Error),
}

pub async fn serve(
    root_dir: impl AsRef<Path>,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ServeError> {
    let root_dir = root_dir.as_ref();
    let db = aries_persistence::connect(root_dir).await?;
    let listener = UnixListener::bind(root_dir.join("aries.sock"))?;

    Server::builder()
        .add_service(AriesServiceServer::new(AriesServiceImpl::new(db)))
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown)
        .await?;
    Ok(())
}

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
        _request: Request<QueryTokenUsagesRequest>,
    ) -> Result<Response<QueryTokenUsagesResponse>, Status> {
        let now = Timestamp::now().to_zoned(TimeZone::UTC);

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
            .map(|r| r.cwd.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let daily =
            records.iter().fold(BTreeMap::<Date, TokenUsage>::new(), |mut daily, record| {
                let date = record.created_at.to_zoned(TimeZone::UTC).date();

                let usage = daily.entry(date).or_default();
                usage.input_tokens = usage.input_tokens.saturating_add(record.input_tokens);
                usage.output_tokens = usage.output_tokens.saturating_add(record.output_tokens);
                usage.total_tokens = usage.total_tokens.saturating_add(record.total_tokens);
                usage.cached_input_tokens =
                    usage.cached_input_tokens.saturating_add(record.cached_input_tokens);
                usage.reasoning_tokens =
                    usage.reasoning_tokens.saturating_add(record.reasoning_tokens);
                daily
            });
        let daily_usages = daily
            .into_iter()
            .map(|(date, usage)| DailyUsage {
                date: Some(date_to_proto(date)),
                token_usage: Some(usage),
            })
            .collect();
        let resp = QueryTokenUsagesResponse { daily_usages, projects };
        Ok(Response::new(resp))
    }
}

fn date_to_proto(date: Date) -> aries_proto::Date {
    aries_proto::Date {
        year: i32::from(date.year()),
        month: date.month() as u32,
        day: date.day() as u32,
    }
}
