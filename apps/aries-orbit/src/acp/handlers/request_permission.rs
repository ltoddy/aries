use agent_client_protocol::v1::schema::{
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Agent, Error};
use async_channel::Sender;
use tokio::sync::{oneshot, watch};

use crate::acp::{Event, Permission};

pub async fn handle(
    request: RequestPermissionRequest,
    responder: Responder<RequestPermissionResponse>,
    connection: ConnectionTo<Agent>,
    events: &Sender<Event>,
    mut cancelled: watch::Receiver<bool>,
) -> Result<(), Error> {
    let (response, answer) = oneshot::channel();
    let permission = Permission::new(
        request.tool_call.fields.title.unwrap_or_else(|| "Agent 请求授权".into()),
        request.options,
        response,
    );
    let already_cancelled = *cancelled.borrow_and_update();
    if already_cancelled || events.send(Event::Permission(permission)).await.is_err() {
        return responder
            .respond(RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled));
    }
    connection.spawn(async move {
        let choice = tokio::select! {
            answer = answer => answer.ok().flatten(),
            _ = cancelled.changed() => None,
        };
        let outcome = choice
            .map(|id| RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id)))
            .unwrap_or(RequestPermissionOutcome::Cancelled);
        responder.respond(RequestPermissionResponse::new(outcome))
    })
}
