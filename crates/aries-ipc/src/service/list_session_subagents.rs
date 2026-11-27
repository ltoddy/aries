use aries_proto::{ListSessionSubagentsRequest, ListSessionSubagentsResponse, Subagent};
use tonic::{Request, Response, Status};

use crate::service::AriesServiceImpl;

impl AriesServiceImpl {
    pub async fn __list_session_subagents(
        &self,
        request: Request<ListSessionSubagentsRequest>,
    ) -> Result<Response<ListSessionSubagentsResponse>, Status> {
        let request = request.into_inner();
        let mut repository = self.subagent_repo.clone();

        let records = repository
            .find_by_session_id(request.session_id)
            .await
            .map_err(|err| Status::internal(format!("failed to find subagents: {err}")))?;

        let subagents = records
            .into_iter()
            .map(|sa| Subagent {
                session_id: sa.session_id,
                agent_id: sa.agent_id,
                name: sa.name,
                description: sa.description,
                last_prompt: sa.last_prompt,
                dir: sa.dir,
                created_at: sa.created_at.as_second(),
            })
            .collect();
        let resp = ListSessionSubagentsResponse { subagents };
        Ok(Response::new(resp))
    }
}
