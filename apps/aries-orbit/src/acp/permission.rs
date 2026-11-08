use agent_client_protocol::v1::schema::{PermissionOption, PermissionOptionId};
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct Permission {
    title: String,
    options: Vec<PermissionOption>,
    response: oneshot::Sender<Option<PermissionOptionId>>,
}

impl Permission {
    pub fn new(
        title: String,
        options: Vec<PermissionOption>,
        response: oneshot::Sender<Option<PermissionOptionId>>,
    ) -> Self {
        Self { title, options, response }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn options(&self) -> &[PermissionOption] {
        &self.options
    }

    pub fn respond(self, option: Option<PermissionOptionId>) {
        let option = option.filter(|id| self.options.iter().any(|option| &option.option_id == id));
        let _ = self.response.send(option);
    }
}
