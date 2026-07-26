pub mod authenticate;
pub mod cancel;
pub mod initialize;
pub mod logout;
pub mod mcp;
pub mod prompt;
pub mod session;

use std::sync::Arc;

use agent_client_protocol::{Agent, ConnectTo, on_receive_notification, on_receive_request};
use aries_init::{GlobalContext, Setting};
use aries_session::SessionRegistry;
use aries_session::session::SessionArgs;
use tokio::sync::Mutex;

use self::authenticate::authenticate;
use self::cancel::cancel;
use self::initialize::initialize;
use self::logout::logout;
use self::prompt::prompt;
use self::session::{
    close_session, delete_session, list_session, load_session, new_session, resume_session,
    set_session_config_option,
};

pub type SharedRegistry = Arc<Mutex<SessionRegistry>>;

pub async fn run(
    gctx: GlobalContext,
    setting: Setting,
    transport: impl ConnectTo<Agent> + 'static,
    bare: bool,
) -> anyhow::Result<()> {
    let registry: SharedRegistry = Arc::new(Mutex::new(SessionRegistry::new(gctx, setting).await?));

    Agent
        .builder()
        .name("aries")
        .on_receive_request(initialize, on_receive_request!())
        .on_receive_request(authenticate, on_receive_request!())
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
                    delete_session(req, responder, cx, registry.clone()).await
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
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    close_session(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(logout, on_receive_request!())
        .on_receive_request(resume_session, on_receive_request!())
        .on_receive_request(
            {
                let registry = registry.clone();
                async move |req, responder, cx| {
                    set_session_config_option(req, responder, cx, registry.clone()).await
                }
            },
            on_receive_request!(),
        )
        .on_receive_notification(
            async move |args, cx| cancel(args, cx, registry.clone()).await,
            on_receive_notification!(),
        )
        .connect_to(transport)
        .await?;

    Ok(())
}
