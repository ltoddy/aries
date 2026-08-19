pub mod authenticate;
pub mod cancel;
pub mod connection;
pub mod initialize;
pub mod logout;
pub mod mcp;
pub mod prompt;
pub mod session;

use agent_client_protocol::{Agent, ConnectTo, on_receive_notification, on_receive_request};
use aries_init::{GlobalContext, Setting};

// WIP

pub async fn run(
    _gctx: GlobalContext,
    _setting: Setting,
    transport: impl ConnectTo<Agent> + 'static,
) -> anyhow::Result<()> {
    Agent
        .v2()
        .name("aries")
        .on_receive_request(initialize::initialize, on_receive_request!())
        .on_receive_request(authenticate::authenticate, on_receive_request!())
        .on_receive_request(logout::logout, on_receive_request!())
        .on_receive_request(session::new_session, on_receive_request!())
        .on_receive_request(session::list_sessions, on_receive_request!())
        .on_receive_request(session::delete_session, on_receive_request!())
        .on_receive_request(prompt::prompt, on_receive_request!())
        .on_receive_request(session::close_session, on_receive_request!())
        .on_receive_request(session::resume_session, on_receive_request!())
        .on_receive_request(session::set_session_config_option, on_receive_request!())
        .on_receive_request(session::fork_session, on_receive_request!())
        .on_receive_notification(cancel::cancel, on_receive_notification!())
        .with_spawned(connection::on_connection_established)
        .on_close(connection::on_connection_closed)
        .connect_to(transport)
        .await?;

    Ok(())
}
