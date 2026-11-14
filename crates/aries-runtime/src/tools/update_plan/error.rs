#[derive(thiserror::Error, Debug)]
pub enum UpdatePlanError {
    #[error("plan entry content cannot be empty")]
    EmptyContent,
    #[error("plan entry active_form cannot be empty")]
    EmptyActiveForm,
}

impl UpdatePlanError {
    pub fn empty_content() -> Self {
        Self::EmptyContent
    }

    pub fn empty_active_form() -> Self {
        Self::EmptyActiveForm
    }
}
