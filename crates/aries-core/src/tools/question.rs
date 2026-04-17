use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, MultiSelect, Select};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionArgs {
    pub question: String,
    pub options: Option<Vec<QuestionOption>>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default = "default_custom")]
    pub custom: bool,
}

fn default_custom() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QuestionOutput {
    pub answers: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum QuestionError {
    #[error("Failed to ask question: {0}")]
    InteractionError(String),
}

pub const NAME: &str = "question";

pub struct QuestionTool;

impl Tool for QuestionTool {
    const NAME: &'static str = NAME;
    type Error = QuestionError;
    type Args = QuestionArgs;
    type Output = QuestionOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/question.txt").to_string(),
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
                labels.push(custom_opt.to_string());
            }

            if args.multiple {
                let selections = MultiSelect::with_theme(&theme)
                    .with_prompt(&args.question)
                    .items(&labels)
                    .interact()
                    .map_err(|e| QuestionError::InteractionError(e.to_string()))?;

                for &idx in &selections {
                    if args.custom && idx == labels.len() - 1 {
                        let custom_answer: String = Input::with_theme(&theme)
                            .with_prompt("Your answer")
                            .interact_text()
                            .map_err(|e| QuestionError::InteractionError(e.to_string()))?;
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
                    .map_err(|e| QuestionError::InteractionError(e.to_string()))?;

                if args.custom && selection == labels.len() - 1 {
                    let custom_answer: String = Input::with_theme(&theme)
                        .with_prompt("Your answer")
                        .interact_text()
                        .map_err(|e| QuestionError::InteractionError(e.to_string()))?;
                    answers.push(custom_answer);
                } else {
                    answers.push(options[selection].label.clone());
                }
            }
        } else {
            let answer: String = Input::with_theme(&theme)
                .with_prompt(&args.question)
                .interact_text()
                .map_err(|e| QuestionError::InteractionError(e.to_string()))?;
            answers.push(answer);
        }

        Ok(QuestionOutput { answers })
    }
}
