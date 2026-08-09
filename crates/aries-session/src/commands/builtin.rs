use std::process::Stdio;

use tokio::process::Command;

use crate::AriesAgentProvider;

const EXIT: &str = "exit";
const SHELL: &str = "shell";
const COMPACT: &str = "compact";
const SYSTEM_PROMPT: &str = "system-prompt";

pub const BUILTIN_COMMANDS: &[(&str, &str, Option<&str>); 4] = &[
    (EXIT, "[builtin] quit the aries process", None),
    (SHELL, "[builtin] run a command in the system shell", Some("command")),
    (COMPACT, "[builtin] compress the conversation context", None),
    (SYSTEM_PROMPT, "[builtin] show the current system prompt", None),
];

pub struct BuiltinCommandsExecutor<'a> {
    session_id: &'a str,
    agent: &'a AriesAgentProvider,
}

impl<'a> BuiltinCommandsExecutor<'a> {
    pub fn new(agent: &'a AriesAgentProvider, session_id: &'a str) -> Self {
        Self { agent, session_id }
    }

    pub fn is_builtin_command(&self, input: impl AsRef<str>) -> bool {
        let input = input.as_ref();
        BUILTIN_COMMANDS.iter().any(|(cmd, _, _)| input == *cmd)
    }

    pub async fn execute(&self, command: impl AsRef<str>, args: impl AsRef<str>) -> bool {
        let command = command.as_ref();
        let args = args.as_ref();

        match command {
            COMPACT => self.compact().await,
            EXIT => self.exit(),
            SHELL => self.shell(args).await,
            SYSTEM_PROMPT => self.system_prompt(),
            _ => return false,
        }
        true
    }

    async fn compact(&self) {}

    fn exit(&self) {
        self.agent.send_notification("Resume this session with:");
        self.agent.send_notification(format!("aries session resume {}", self.session_id));
        std::process::exit(0);
    }

    async fn shell(&self, args: impl AsRef<str>) {
        let args = args.as_ref();
        let shell = std::env::var("SHELL").unwrap_or(String::from("bash"));

        let output = Command::new(shell)
            .arg("-c")
            .arg(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .await;
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.agent.send_notification("```");
                if !stdout.is_empty() {
                    self.agent.send_notification(stdout);
                }
                if !stderr.is_empty() {
                    self.agent.send_notification(stderr);
                }
                self.agent.send_notification("```");
            },
            Err(err) => {
                self.agent.send_notification(err.to_string());
            },
        }
    }

    fn system_prompt(&self) {
        self.agent.send_notification("```");
        self.agent.send_notification(self.agent.preamble());
        self.agent.send_notification("```");
    }
}
