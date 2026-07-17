#[derive(thiserror::Error, Debug)]
pub enum UpdatePlanError {
    #[error("Plan entry content cannot be empty")]
    EmptyContent,
    #[error("Plan entry active_form cannot be empty")]
    EmptyActiveForm,
}
