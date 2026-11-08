pub mod auth;
pub mod cancel;
pub mod initialize;
pub mod logout;
pub mod mcp;
pub mod prompt;
pub mod provider;
pub mod session;

use std::sync::Arc;

use agent_client_protocol::{Agent, ConnectTo, on_receive_notification, on_receive_request};
use aries_init::{GlobalContext, Setting};
use aries_runtime::session::{SessionArgs, SessionRegistry};
use tokio::sync::Mutex;

use self::auth::{auth_status, authenticate};
use self::cancel::{cancel, cancel_request};
use self::initialize::initialize;
use self::logout::logout;
use self::prompt::prompt;
use self::provider::{disable_provider, list_providers, set_provider};
use self::session::{
    archive_session, close_session, delete_session, fork_session, list_session, load_session,
    new_session, resume_session, session_status, set_session_config_option, set_session_mode,
    set_session_title, unarchive_session,
};

pub async fn run(
    gctx: GlobalContext,
    setting: Setting,
    transport: impl ConnectTo<Agent> + 'static,
    bare: bool,
) -> Result<(), agent_client_protocol::Error> {
    let registry: aries_runtime::session::SharedRegistry =
        Arc::new(Mutex::new(SessionRegistry::new(gctx, setting).await?));

    Agent
        .v1()
        .name("aries")
        .on_receive_request(initialize, on_receive_request!())
        .on_receive_request(authenticate, on_receive_request!())
        .on_receive_request(auth_status, on_receive_request!())
        .on_receive_request(logout, on_receive_request!())
        .on_receive_request(list_providers, on_receive_request!())
        .on_receive_request(set_provider, on_receive_request!())
        .on_receive_request(disable_provider, on_receive_request!())
        .on_receive_request(
            {
                let register = registry.clone();
                async move |req, responder, cx| {
                    let session_args = SessionArgs::new(bare);
                    new_session(req, responder, cx, register.clone(), session_args).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    load_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    list_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    fork_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(resume_session, on_receive_request!())
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    close_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    delete_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(archive_session, on_receive_request!())
        .on_receive_request(unarchive_session, on_receive_request!())
        .on_receive_request(session_status, on_receive_request!())
        .on_receive_request(set_session_title, on_receive_request!())
        .on_receive_request(set_session_mode, on_receive_request!())
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    set_session_config_option(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| prompt(req, responder, cx, registry.clone()).await
            },
            on_receive_request!(),
        )
        .on_receive_request(mcp::message, on_receive_request!())
        .on_receive_notification(cancel_request, on_receive_notification!())
        .on_receive_notification(
            async move |args, cx| cancel(args, cx, registry.clone()).await,
            on_receive_notification!(),
        )
        .on_receive_notification(mcp::message_notification, on_receive_notification!())
        .connect_to(transport)
        .await?;

    Ok(())
}
