pub mod bash;
pub mod codesearch;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod multiedit;
pub mod question;
pub mod read;
pub mod webfetch;
pub mod websearch;
pub mod write;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("failed to deserialize tool data: {0}")]
    Deserialize(#[from] serde_json::Error),
}

pub trait ToolArgsRender {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError>;
}

pub trait ToolOutputRender {
    fn render_output(raw: &str) -> Result<String, RenderError>;
}
