mod args;
mod output;

use std::convert::Infallible;

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, MultiSelect, Select};
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{AskUserQuestionArgs, AskUserQuestionOption};
pub use self::output::AskUserQuestionOutput;

pub const NAME: &str = "AskUserQuestion";

pub struct AskUserQuestionTool;

impl Default for AskUserQuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AskUserQuestionTool {
    pub fn new() -> Self {
        Self {}
    }

    pub fn ask(&self, args: &AskUserQuestionArgs) -> Result<Vec<String>, dialoguer::Error> {
        let theme = ColorfulTheme::default();
        let mut answers = Vec::new();

        if let Some(options) = &args.options {
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
                    .interact()?;

                for &idx in &selections {
                    if args.custom && idx == labels.len() - 1 {
                        let custom_answer: String =
                            Input::with_theme(&theme).with_prompt("Your answer").interact_text()?;
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
                    .interact()?;

                if args.custom && selection == labels.len() - 1 {
                    let custom_answer: String =
                        Input::with_theme(&theme).with_prompt("Your answer").interact_text()?;
                    answers.push(custom_answer);
                } else {
                    answers.push(options[selection].label.clone());
                }
            }
        } else {
            let answer: String =
                Input::with_theme(&theme).with_prompt(&args.question).interact_text()?;
            answers.push(answer);
        }

        Ok(answers)
    }
}

impl Tool for AskUserQuestionTool {
    const NAME: &'static str = NAME;
    type Args = AskUserQuestionArgs;
    type Output = AskUserQuestionOutput;
    type Error = Infallible;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
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
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        _args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        Ok(AskUserQuestionOutput::new())
    }
}
