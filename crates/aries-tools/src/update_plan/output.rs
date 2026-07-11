use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanOutput {
    pub ok: bool,
}

impl ToolOutputRender for UpdatePlanOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let _: Self = serde_json::from_str(raw)?;
        Ok("Plan updated.".to_owned())
    }
}
