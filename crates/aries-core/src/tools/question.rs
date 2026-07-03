use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, MultiSelect, Select};
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};

use super::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionArgs {
    pub question: String,
    pub options: Option<Vec<AskUserQuestionOption>>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default = "default_custom")]
    pub custom: bool,
}

impl AskUserQuestionArgs {
    pub fn title(&self) -> String {
        format!("Ask user: {}", self.question)
    }
}

impl ToolArgsRender for AskUserQuestionArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.question;
        Ok((first, None))
    }
}

fn default_custom() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOutput {
    pub answers: Vec<String>,
}

impl ToolOutputRender for AskUserQuestionOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.answers.join("\n"))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AskUserQuestionError {
    #[error("Failed to ask question: {0}")]
    InteractionError(String),
}

pub const NAME: &str = "AskUserQuestion";

pub struct AskUserQuestionTool;

impl Tool for AskUserQuestionTool {
    const NAME: &'static str = NAME;
    type Error = AskUserQuestionError;
    type Args = AskUserQuestionArgs;
    type Output = AskUserQuestionOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("question.md").to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user"
                    },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["label"]
                        }
                    },
                    "multiple": {
                        "type": "boolean",
                        "description": "Allow selecting multiple options"
                    },
                    "custom": {
                        "type": "boolean",
                        "description": "Allow the user to type a custom answer (default true)"
                    }
                },
                "required": ["question"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut answers = Vec::new();
        let theme = ColorfulTheme::default();

        if let Some(options) = args.options {
            let mut labels: Vec<String> = options
                .iter()
                .map(|o| {
                    if let Some(desc) = &o.description {
                        format!("{} - {}", o.label, desc)
                    } else {
                        o.label.clone()
                    }
                })
                .collect();

            let custom_opt = "Type your own answer...";
            if args.custom {
                labels.push(custom_opt.to_owned());
            }

            if args.multiple {
                let selections = MultiSelect::with_theme(&theme)
                    .with_prompt(&args.question)
                    .items(&labels)
                    .interact()
                    .map_err(|e| AskUserQuestionError::InteractionError(e.to_string()))?;

                for &idx in &selections {
                    if args.custom && idx == labels.len() - 1 {
                        let custom_answer: String = Input::with_theme(&theme)
                            .with_prompt("Your answer")
                            .interact_text()
                            .map_err(|e| AskUserQuestionError::InteractionError(e.to_string()))?;
                        answers.push(custom_answer);
                    } else {
                        answers.push(options[idx].label.clone());
                    }
                }
            } else {
                let selection = Select::with_theme(&theme)
                    .with_prompt(&args.question)
                    .items(&labels)
                    .default(0)
                    .interact()
                    .map_err(|e| AskUserQuestionError::InteractionError(e.to_string()))?;

                if args.custom && selection == labels.len() - 1 {
                    let custom_answer: String = Input::with_theme(&theme)
                        .with_prompt("Your answer")
                        .interact_text()
                        .map_err(|e| AskUserQuestionError::InteractionError(e.to_string()))?;
                    answers.push(custom_answer);
                } else {
                    answers.push(options[selection].label.clone());
                }
            }
        } else {
            let answer: String = Input::with_theme(&theme)
                .with_prompt(&args.question)
                .interact_text()
                .map_err(|e| AskUserQuestionError::InteractionError(e.to_string()))?;
            answers.push(answer);
        }

        Ok(AskUserQuestionOutput { answers })
    }
}
