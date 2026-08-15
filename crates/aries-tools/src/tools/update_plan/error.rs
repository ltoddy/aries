#[derive(thiserror::Error, Debug)]
pub enum UpdatePlanError {
    #[error("plan entry content cannot be empty")]
    EmptyContent,
    #[error("plan entry active_form cannot be empty")]
    EmptyActiveForm,
}
