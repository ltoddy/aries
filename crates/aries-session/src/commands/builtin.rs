use aries_agent::AriesAgent;
use aries_compact::ContextCompactor;
use aries_event::Notifier;
use tokio::process::Command;

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
    agent: &'a AriesAgent,
    compactor: ContextCompactor,
    notifier: Notifier,
}

impl<'a> BuiltinCommandsExecutor<'a> {
    pub fn new(
        agent: &'a AriesAgent,
        session_id: &'a str,
        compactor: ContextCompactor,
        notifier: Notifier,
    ) -> Self {
        Self { agent, session_id, compactor, notifier }
    }

    pub fn is_builtin_command(&self, input: impl AsRef<str>) -> bool {
        let input = input.as_ref();
        BUILTIN_COMMANDS.iter().any(|(cmd, _, _)| input == *cmd)
    }

    pub async fn execute(&mut self, command: impl AsRef<str>, args: impl AsRef<str>) -> bool {
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

    async fn compact(&mut self) {
        self.compactor.compact().await
    }

    fn exit(&self) {
        self.notifier.notify("Resume this session with:");
        self.notifier.notify(format!("aries session resume {}", self.session_id));
        std::process::exit(0);
    }

    async fn shell(&self, args: impl AsRef<str>) {
        let args = args.as_ref();
        let shell = std::env::var("SHELL").unwrap_or(String::from("bash"));

        let output = Command::new(shell).arg("-c").arg(args).output().await;
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.notifier.notify("```");
                if !stdout.is_empty() {
                    self.notifier.notify(stdout);
                }
                if !stderr.is_empty() {
                    self.notifier.notify(stderr);
                }
                self.notifier.notify("```");
            },
            Err(err) => {
                self.notifier.notify(err.to_string());
            },
        }
    }

    fn system_prompt(&self) {
        self.notifier.notify("```");
        self.notifier.notify(self.agent.preamble());
        self.notifier.notify("```");
    }
}
