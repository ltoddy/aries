use std::fs;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

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
        if !patch_text.starts_with("*** Begin Patch") || !patch_text.ends_with("*** End Patch") {
            return Err(ApplyPatchError::PatchError(
                "Patch must start with '*** Begin Patch' and end with '*** End Patch'".to_string(),
            ));
        }

        let lines: Vec<&str> = patch_text.lines().collect();
        let mut current_file = String::new();
        let mut action = String::new();
        let mut new_content = String::new();

        let mut i = 1; // Skip "*** Begin Patch"
        while i < lines.len() - 1 {
            // Skip "*** End Patch"
            let line = lines[i];

            if line.starts_with("*** Add File: ") {
                current_file = line.trim_start_matches("*** Add File: ").trim().to_string();
                action = "Add".to_string();
                new_content.clear();
            } else if line.starts_with("*** Delete File: ") {
                let file_to_delete = line.trim_start_matches("*** Delete File: ").trim();
                if let Err(e) = fs::remove_file(file_to_delete) {
                    return Err(ApplyPatchError::PatchError(format!(
                        "Failed to delete file {}: {}",
                        file_to_delete, e
                    )));
                }
                println!("{} {}", "Deleted".red().bold(), file_to_delete);
            } else if line.starts_with("*** Update File: ") {
                current_file = line.trim_start_matches("*** Update File: ").trim().to_string();
                action = "Update".to_string();

                // For a simple MVP, if it's an update, we expect the LLM to just provide the
                // full new content A full diff parser (like opencode's) is
                // complex, so we simplify it here for the MVP. We'll just read
                // the new lines until the next header.
                new_content.clear();
            } else if line.starts_with("@@") {
                // Ignore context lines for this simplified MVP
            } else if line.starts_with("+") {
                if action == "Add" || action == "Update" {
                    new_content.push_str(&line[1..]);
                    new_content.push('\n');
                }
            } else if line.starts_with("-") {
                // For our simplified MVP, we just build the new file from +
                // lines
            } else if !line.starts_with("***") {
                // Unchanged lines
                if action == "Update" {
                    new_content.push_str(line);
                    new_content.push('\n');
                }
            }

            // If we hit the next file header or end of patch, apply the previous one
            if (i == lines.len() - 2 || lines[i + 1].starts_with("*** ")) && !current_file.is_empty() {
                if action == "Add" || action == "Update" {
                    if let Some(parent) = Path::new(&current_file).parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if let Err(e) = fs::write(&current_file, &new_content) {
                        return Err(ApplyPatchError::PatchError(format!("Failed to write to {}: {}", current_file, e)));
                    }
                    if action == "Add" {
                        println!("{} {}", "Created".green().bold(), current_file);
                    } else {
                        println!("{} {}", "Updated".yellow().bold(), current_file);
                    }
                }
                current_file.clear();
                action.clear();
            }
            i += 1;
        }

        Ok(ApplyPatchOutput { success: true, message: "Patch applied successfully".to_string() })
    }
}
