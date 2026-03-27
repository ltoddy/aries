use std::path::Path;

use anyhow::Result;
use colored::Colorize;
use diffy::{Patch, apply};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Deserialize)]
pub struct ApplyPatchArgs {
    patch: String,
}

#[derive(Serialize)]
pub struct ApplyPatchOutput {
    success: bool,
    message: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ApplyPatchError {
    #[error("Failed to apply patch: {0}")]
    PatchError(String),
}

pub struct ApplyPatchTool;

impl Tool for ApplyPatchTool {
    const NAME: &'static str = "apply_patch";
    type Error = ApplyPatchError;
    type Args = ApplyPatchArgs;
    type Output = ApplyPatchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/apply_patch.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "The patch content to apply"
                    }
                },
                "required": ["patch"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let patch_text = args.patch.trim();

        if patch_text.starts_with("*** Begin Patch") {
            return Err(ApplyPatchError::PatchError("Legacy simplified patch format is no longer supported. Please provide standard unified diff format starting with '--- a/file' and '+++ b/file'".to_string()));
        }

        let patch = Patch::from_str(patch_text)
            .map_err(|e| ApplyPatchError::PatchError(format!("Failed to parse unified diff: {}", e)))?;

        let mut old_file = String::new();
        let mut new_file = String::new();
        for line in patch_text.lines() {
            if let Some(stripped) = line.strip_prefix("--- ") {
                old_file = stripped.split('\t').next().unwrap_or("").trim().to_string();
                if old_file.starts_with("a/") {
                    old_file = old_file[2..].to_string();
                }
            } else if let Some(stripped) = line.strip_prefix("+++ ") {
                new_file = stripped.split('\t').next().unwrap_or("").trim().to_string();
                if new_file.starts_with("b/") {
                    new_file = new_file[2..].to_string();
                }
                break;
            }
        }

        let old_path = Path::new(&old_file);
        let new_path = Path::new(&new_file);

        if new_file.is_empty() || new_file == "/dev/null" {
            if old_path.exists() {
                fs::remove_file(old_path)
                    .await
                    .map_err(|e| ApplyPatchError::PatchError(format!("Failed to delete file {}: {}", old_file, e)))?;
                println!("{} {}", "Deleted".red().bold(), old_file);
            }
            return Ok(ApplyPatchOutput { success: true, message: format!("Deleted file {}", old_file) });
        }

        let original_content = if old_path.exists() && old_file != "/dev/null" {
            fs::read_to_string(old_path)
                .await
                .map_err(|e| ApplyPatchError::PatchError(format!("Failed to read file {}: {}", old_file, e)))?
        } else {
            String::new()
        };

        let new_content = apply(&original_content, &patch)
            .map_err(|e| ApplyPatchError::PatchError(format!("Failed to apply patch to {}: {}", new_file, e)))?;

        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ApplyPatchError::PatchError(format!("Failed to create directories for {}: {}", new_file, e))
            })?;
        }

        fs::write(new_path, new_content)
            .await
            .map_err(|e| ApplyPatchError::PatchError(format!("Failed to write to {}: {}", new_file, e)))?;

        if !original_content.is_empty() {
            println!("{} {}", "Updated".yellow().bold(), new_file);
        } else {
            println!("{} {}", "Created".green().bold(), new_file);
        }

        Ok(ApplyPatchOutput { success: true, message: format!("Successfully applied patch to {}", new_file) })
    }
}
