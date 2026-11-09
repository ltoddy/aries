use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::SessionNotification;
use async_channel::Sender;

use crate::acp::Event;

pub async fn handle(
    notification: SessionNotification,
    events: &Sender<Event>,
) -> Result<(), Error> {
    let _ = events.send(Event::Update(Box::new(notification))).await;
    Ok(())
}
