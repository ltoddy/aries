use std::path::PathBuf;

use aries_extension::agent::AgentDefinition;
use rig::Model;
use rig::completion::Effort;
use rig::providers::openai::wire::OpenAiWire;

#[derive(Clone, Debug)]
pub struct State {
    pub agent: AgentDefinition,
    pub effort: Effort,
    pub turns: MaxTurn,
    pub model: Model<OpenAiWire>,
    pub cwd: PathBuf,
    pub session_dir: PathBuf,
    pub session_id: String,
}

impl State {
    pub fn new(
        model: Model<OpenAiWire>,
        agent: AgentDefinition,
        cwd: impl Into<PathBuf>,
        session_dir: impl Into<PathBuf>,
        session_id: impl Into<String>,
    ) -> Self {
        let cwd = cwd.into();
        let session_dir = session_dir.into();
        let session_id = session_id.into();

        Self {
            effort: Effort::High,
            turns: MaxTurn::Turns400,
            model,
            agent,
            cwd,
            session_dir,
            session_id,
        }
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
    pub fn agent(&mut self, agent: AgentDefinition) {
        self.agent = agent;
    }

    pub fn effort(&mut self, effort: Effort) {
        self.effort = effort;
    }

    pub fn turns(&mut self, turns: MaxTurn) {
        self.turns = turns;
    }

    pub fn model(&mut self, model: Model<OpenAiWire>) {
        self.model = model;
    }

    pub fn cwd(&mut self, cwd: impl Into<PathBuf>) {
        let cwd = cwd.into();
        self.cwd = cwd;
    }
}
