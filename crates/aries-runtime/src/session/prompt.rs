use aries_communicate::AgentEvent;
use jiff::Zoned;
use rig::agent::PromptResponse;
use rig::completion::Message;
use rig::message::UserContent;
use tokio::pin;
use tokio_util::sync::CancellationToken;

use crate::session::{PromptOutcome, Session, system_reminder, system_reminders};

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
}
