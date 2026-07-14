use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteOutput {
    pub success: bool,
}

impl WriteOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(if output.success {
            "File written successfully".to_owned()
        } else {
            "Failed to write file".to_owned()
        })
    }
}
