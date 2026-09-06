use agent_client_protocol::{Client, Error, V2ConnectionTo};
use tracing::info;

pub async fn on_connection_established(_cx: V2ConnectionTo<Client>) -> Result<(), Error> {
    info!("aries agent v2 connection established");
    Ok(())
}

pub async fn on_connection_closed(_cx: V2ConnectionTo<Client>) -> Result<(), Error> {
    info!("aries agent v2 connection closed");
    Ok(())
}
