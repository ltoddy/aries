use std::collections::VecDeque;

use aries_communicate::AgentEvent;
use aries_extension::command::SlashCommandsExecutor;
use aries_extension::hook::HookDecision;
use aries_extension::hook::input::{
    StopFailureHookInput, StopHookInput, UserPromptSubmitHookInput,
};
use aries_extension::skill::SkillExecutor;
use jiff::Zoned;
use rig::agent::PromptResponse;
use rig::completion::Message;
use rig::message::UserContent;
use tokio::pin;
use tokio_util::sync::CancellationToken;

use crate::memory::MemoryRetriever;
use crate::session::{
    PromptOutcome, Session, message_to_simple_text, system_reminder, system_reminders,
};

impl Session {
    #[tracing::instrument(name = "prompt", skip_all, fields(session_id = %self.state.session_id))]
    pub async fn prompt<F, Fut>(
        &mut self,
        prompt: impl Into<Message>,
        mut callback: F,
    ) -> crate::agent::AriesResult<PromptOutcome>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut prompt: Message = prompt.into();
        let cancel_token = {
            let mut guard = self.cancel_token.lock();
            *guard = CancellationToken::new();
            guard.clone()
        };
        self.last_assistant_message = None;
        let message_id = format!("user-{}", nanoid::nanoid!());

        if let Message::User { ref content } = prompt
            && let Some(UserContent::Text(text)) = content.first()
            && let Some(input) = text.text.trim().strip_prefix("/")
            && let Some(slash_command) = self.try_execute_slash_command(input).await
        {
            prompt = Message::user(slash_command);
        }

        let title = self.update_title(&prompt).await;
        let now = Zoned::now();
        self.notifier.send_session_info_update(&title, now.to_string());

        let hook_context = self.fire_user_prompt_submit(&title).await?;
        self.compactor.pre_compact(&prompt).await;

        let context = self.recall_context(&title).await;
        let mut history = self.chat_context.history().await.to_vec();
        history.extend(system_reminders(hook_context));
        if let Some(reminder) = context {
            history.push(system_reminder(reminder));
        }
        let final_res = {
            let future = self.agent.prompt(prompt, history);
            pin!(future);

            let mut guard = self.receiver.lock().await;
            let mut final_res = Ok(PromptResponse::empty());
            let mut cancelled = false;
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => {
                        cancelled = true;
                        break;
                    }
                    event = guard.recv() => {
                        if let Some(event) = event {
                            callback(event).await;
                        }
                    }
                    res = &mut future => {
                        final_res = res;
                        break;
                    }
                }
            }
            while let Ok(event) = guard.try_recv() {
                callback(event).await;
            }
            drop(guard);

            if cancelled {
                return Ok(PromptOutcome::Cancelled(message_id));
            }

            match final_res {
                Ok(res) => res,
                Err(err) => {
                    self.fire_stop_failure(err.to_string()).await;
                    return Err(err);
                },
            }
        };

        let output = final_res.output();
        self.last_assistant_message = Some(output.clone());
        let messages = final_res.messages();
        self.append_messages(messages).await;
        self.fire_stop(&output).await;
        if let Some(completion) = final_res.completion_calls.last() {
            self.compactor.post_compact(completion.usage).await;
        }

        Ok(PromptOutcome::Completed(message_id))
    }

    async fn fire_user_prompt_submit(
        &self,
        prompt: impl Into<String>,
    ) -> crate::agent::AriesResult<VecDeque<String>> {
        let input = UserPromptSubmitHookInput::new(&self.state.session_id, &self.state.cwd, prompt)
            .transcript_dir(&self.state.transcript_dir);
        match self.hooks_executor.fire_user_prompt_submit(input).await {
            HookDecision::Terminate { reason } => {
                Err(crate::agent::AriesError::hook_terminated(reason))
            },
            HookDecision::Continue { contexts } => Ok(contexts),
        }
    }

    async fn fire_stop_failure(&self, error: impl Into<String>) {
        let error = error.into();
        let input = StopFailureHookInput::new(&self.state.session_id, &self.state.cwd, &error)
            .transcript_dir(&self.state.transcript_dir)
            .error_details(&error);

        let input = match &self.last_assistant_message {
            Some(message) => input.last_assistant_message(message),
            None => input,
        };

        self.hooks_executor.fire_stop_failure(input).await;
    }

    async fn fire_stop(&self, assistant_output: impl Into<String>) {
        let input = StopHookInput::new(&self.state.session_id, &self.state.cwd, false)
            .last_assistant_message(assistant_output);
        if let HookDecision::Continue { contexts } = self.hooks_executor.fire_stop(input).await {
            self.chat_context.append(&system_reminders(contexts)).await;
        }
    }

    async fn update_title(&mut self, prompt: &Message) -> String {
        let title = message_to_simple_text(prompt);
        let _ = self.session_repo.update_title_by_session_id(&self.state.session_id, &title).await;
        title
    }

    async fn try_execute_slash_command(&self, input: impl AsRef<str>) -> Option<String> {
        let input = input.as_ref();
        let executor = SlashCommandsExecutor::new(&self.extensions.commands);
        if let Some(command) = executor.execute(input).await {
            return Some(command);
        }

        let executor = SkillExecutor::new(&self.extensions.skills);
        executor.execute(input).await
    }

    async fn recall_context(&self, query: impl AsRef<str>) -> Option<String> {
        let query = query.as_ref();
        let memories = self.memory_store.scan().await;
        let retriever = MemoryRetriever::new(self.state.model.clone());
        let retrieved = retriever.retrieve(query, &memories).await;

        let mut blocks = Vec::<String>::with_capacity(retrieved.len());
        for file_name in retrieved {
            if let Some(body) = self.memory_store.read_memory(&file_name).await {
                blocks.push(format!("## {file_name}\n\n{body}"));
            }
        }

        if blocks.is_empty() {
            return None;
        }
        Some(format!(
            "以下是与当前对话相关的历史记忆（自动召回，用户不可见）：\n\n{}",
            blocks.join("\n\n")
        ))
    }

    async fn append_messages(&self, messages: impl AsRef<[Message]>) {
        let messages = messages.as_ref();
        if messages.is_empty() {
            return;
        }

        self.chat_history.append(messages).await;
        self.chat_context.append(messages).await;
    }
}
