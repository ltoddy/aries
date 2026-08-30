pub mod list;
pub mod prune;
pub mod resume;
pub mod run;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use aries_event::AgentEvent;
use aries_session::{Session, resume_input};
use aries_tools::question::{AskUserQuestionArgs, AskUserQuestionTool};
use clap::Subcommand;
use colored::Colorize;
use parking_lot::Mutex;
use terminal_size::{Width, terminal_size};
use tracing::warn;

use self::list::ListSessionsArgs;
use self::prune::PruneSessionsArgs;
use self::resume::ResumeSessionsArgs;
use crate::display::print_agent_event;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    #[command(about = "List chat sessions")]
    List(ListSessionsArgs),
    #[command(about = "Delete old chat sessions")]
    Prune(PruneSessionsArgs),
    #[command(about = "Resume a previous chat session")]
    Resume(ResumeSessionsArgs),
}

pub async fn prompt_maybe_ask(
    session: &mut Session,
    input: impl Into<String>,
) -> anyhow::Result<()> {
    let mut prompt_input = input.into();

    loop {
        let tool_names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let question: Arc<Mutex<Option<AskUserQuestionArgs>>> = Arc::new(Mutex::new(None));

        let callback = {
            let tool_names = tool_names.clone();
            let question = question.clone();
            move |event: AgentEvent| {
                let tool_names = tool_names.clone();
                let question = question.clone();
                async move {
                    if let AgentEvent::AwaitingUserInput { args } = &event {
                        let mut slot = question.lock();
                        match serde_json::from_value::<AskUserQuestionArgs>(args.clone()) {
                            Ok(args) => *slot = Some(args),
                            Err(err) => warn!("failed to parse AskUserQuestion args: {err}"),
                        }
                    }
                    let mut map = tool_names.lock();
                    print_agent_event(event, &mut map);
                }
            }
        };

        session.prompt(prompt_input.clone(), callback).await.map_err(anyhow::Error::from)?;

        match question.lock().take() {
            Some(pending) => {
                let answers = AskUserQuestionTool::new().ask(&pending)?;
                prompt_input = resume_input(&pending.question, &answers.join("\n"));
            },
            None => return Ok(()),
        }
    }
}

fn display_elapsed(start: Instant) {
    let elapsed = start.elapsed();
    let terminal_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    let prefix = "─".repeat(5);
    let time = format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64());
    let remining_width = terminal_width.saturating_sub(prefix.len() + time.len());
    let line = format!("{}{}{}", "─".repeat(5), time, "─".repeat(remining_width));
    println!("{}\n", line.dimmed());
}
