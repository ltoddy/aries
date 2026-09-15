pub mod authenticate;
pub mod cancel;
pub mod connection;
pub mod initialize;
pub mod logout;
pub mod mcp;
pub mod prompt;
pub mod session;

use std::sync::Arc;

use agent_client_protocol::{Agent, ConnectTo, on_receive_notification, on_receive_request};
use aries_init::{GlobalContext, Setting};
use aries_session::{SessionArgs, SessionRegistry};
use tokio::sync::Mutex;

pub async fn run(
    gctx: GlobalContext,
    setting: Setting,
    transport: impl ConnectTo<Agent> + 'static,
) -> Result<(), agent_client_protocol::Error> {
    let registry: aries_session::SharedRegistry =
        Arc::new(Mutex::new(SessionRegistry::new(gctx, setting).await?));
    let session_args = SessionArgs::default();

    Agent
        .v2()
        .name("aries")
        .on_receive_request(initialize::initialize, on_receive_request!())
        .on_receive_request(authenticate::authenticate, on_receive_request!())
        .on_receive_request(logout::logout, on_receive_request!())
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::new_session(req, responder, cx, registry.clone(), session_args.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::list_sessions(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::delete_session(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::close_session(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::resume_session(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(session::set_session_mode, on_receive_request!())
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::set_session_config_option(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| {
                    session::fork_session(req, responder, cx, registry.clone())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                move |req, responder, cx| prompt::prompt(req, responder, cx, registry.clone())
            },
            on_receive_request!(),
        )
        .on_receive_request(mcp::connect, on_receive_request!())
        .on_receive_request(mcp::message, on_receive_request!())
        .on_receive_request(mcp::disconnect, on_receive_request!())
        .on_receive_notification(cancel::cancel_request, on_receive_notification!())
        .on_receive_notification(
            {
                let registry = registry.clone();
                move |notif, cx| cancel::cancel(notif, cx, registry.clone())
            },
            on_receive_notification!(),
        )
        .on_receive_notification(mcp::message_notification, on_receive_notification!())
        .with_spawned(connection::on_connection_established)
        .on_close(connection::on_connection_closed)
        .connect_to(transport)
        .await?;

    Ok(())
}
