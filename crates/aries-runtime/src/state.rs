use std::path::PathBuf;
use std::sync::Arc;

use aries_extension::agent::AgentDefinition;
use parking_lot::RwLock;
use rig::Model;
use rig::completion::Effort;
use rig::providers::openai::wire::OpenAiWire;

use crate::sandbox::SandboxMode;

#[derive(Clone, Debug)]
pub struct State {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug)]
struct Inner {
    agent: AgentDefinition,
    effort: Effort,
    turns: MaxTurn,
    model: Model<OpenAiWire>,
    cwd: PathBuf,
    session_dir: PathBuf,
    session_id: String,
    transcript_dir: PathBuf,
    sandbox_mode: SandboxMode,
}

impl Inner {
    fn new(
        model: Model<OpenAiWire>,
        agent: AgentDefinition,
        cwd: impl Into<PathBuf>,
        session_dir: impl Into<PathBuf>,
        session_id: impl Into<String>,
        transcript_dir: impl Into<PathBuf>,
    ) -> Self {
        let cwd = cwd.into();
        let session_dir = session_dir.into();
        let session_id = session_id.into();
        let transcript_dir = transcript_dir.into();
        let sandbox_mode = SandboxMode::default();

        Self {
            effort: Effort::High,
            turns: MaxTurn::default(),
            model,
            agent,
            cwd,
            session_dir,
            session_id,
            transcript_dir,
            sandbox_mode,
        }
    }
}

impl State {
    pub fn new(
        model: Model<OpenAiWire>,
        agent: AgentDefinition,
        cwd: impl Into<PathBuf>,
        session_dir: impl Into<PathBuf>,
        session_id: impl Into<String>,
        transcript_dir: impl Into<PathBuf>,
    ) -> Self {
        let inner = Inner::new(model, agent, cwd, session_dir, session_id, transcript_dir);
        Self { inner: Arc::new(RwLock::new(inner)) }
    }

    pub fn agent(&self) -> AgentDefinition {
        self.inner.read().agent.clone()
    }

    pub fn effort(&self) -> Effort {
        self.inner.read().effort
    }

    pub fn turns(&self) -> MaxTurn {
        self.inner.read().turns
    }

    pub fn model(&self) -> Model<OpenAiWire> {
        self.inner.read().model.clone()
    }

    pub fn cwd(&self) -> PathBuf {
        self.inner.read().cwd.clone()
    }

    pub fn session_dir(&self) -> PathBuf {
        self.inner.read().session_dir.clone()
    }

    pub fn session_id(&self) -> String {
        self.inner.read().session_id.clone()
    }

    pub fn transcript_dir(&self) -> PathBuf {
        self.inner.read().transcript_dir.clone()
    }

    pub fn sandbox_mode(&self) -> SandboxMode {
        self.inner.read().sandbox_mode
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum MaxTurn {
    Turns100,
    Turns200,
    #[default]
    Turns400,
    Turns800,
}

impl MaxTurn {
    pub fn as_usize(self) -> usize {
        match self {
            MaxTurn::Turns100 => 100,
            MaxTurn::Turns200 => 200,
            MaxTurn::Turns400 => 400,
            MaxTurn::Turns800 => 800,
        }
    }
}

impl State {
    pub fn set_agent(&self, agent: AgentDefinition) {
        self.inner.write().agent = agent;
    }

    pub fn set_effort(&self, effort: Effort) {
        self.inner.write().effort = effort;
    }

    pub fn set_turns(&self, turns: MaxTurn) {
        self.inner.write().turns = turns;
    }

    pub fn set_model(&self, model: Model<OpenAiWire>) {
        self.inner.write().model = model;
    }

    pub fn set_cwd(&self, cwd: impl Into<PathBuf>) {
        let cwd = cwd.into();
        self.inner.write().cwd = cwd;
    }

    pub fn set_sandbox_mode(&self, sandbox_mode: SandboxMode) {
        self.inner.write().sandbox_mode = sandbox_mode;
    }
}
