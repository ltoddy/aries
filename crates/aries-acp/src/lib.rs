pub mod authenticate;
pub mod cancel;
pub mod initialize;
pub mod logout;
pub mod prompt;
pub mod session;

use std::sync::Arc;

use agent_client_protocol::{Agent, ConnectTo, on_receive_notification, on_receive_request};
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_session::SessionRegistry;
use tokio::sync::Mutex;

use crate::authenticate::authenticate;
use crate::cancel::cancel;
use crate::initialize::initialize;
use crate::logout::logout;
use crate::prompt::prompt;
use crate::session::{
    close_session, list_session, load_session, new_session, resume_session,
    set_session_config_option, set_session_mode,
};

type SharedRegistry = Arc<Mutex<SessionRegistry>>;

pub async fn run(
    gctx: GlobalContext,
    config: AriesConfig,
    transport: impl ConnectTo<Agent> + 'static,
) -> anyhow::Result<()> {
    let registry: SharedRegistry = Arc::new(Mutex::new(SessionRegistry::new(gctx, config).await?));

    Agent
        .builder()
        .name("aries")
        .on_receive_request(initialize, on_receive_request!())
        .on_receive_request(authenticate, on_receive_request!())
        .on_receive_request(
            {
                let register = registry.clone();
                async move |req, responder, cx| {
                    new_session(req, responder, cx, register.clone()).await
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
                    set_session_mode(req, responder, cx, registry.clone()).await
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
        .on_receive_request(close_session, on_receive_request!())
        .on_receive_request(logout, on_receive_request!())
        .on_receive_request(resume_session, on_receive_request!())
        .on_receive_request(set_session_config_option, on_receive_request!())
        .on_receive_notification(
            async move |args, cx| cancel(args, cx, registry.clone()).await,
            on_receive_notification!(),
        )
        .connect_to(transport)
        .await?;

    Ok(())
}
