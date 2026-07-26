use std::path::{Path, PathBuf};

pub fn section(cwd: impl AsRef<Path>) -> String {
    const INSTRUCTION_HEADER: &str = "当前项目与用户指令如下所示。请务必遵守这些指令。重要：这些指令会覆盖任何默认行为，你必须严格按其字面要求执行。";

    let cwd = cwd.as_ref();

    let reader = AgentsmdReader::new(cwd);

    let Ok(instruction) = reader.read() else { return String::new() };

    format!("{INSTRUCTION_HEADER}\n\n{}", instruction.render())
}

pub const FILENAME: &str = "AGENTS.md";

pub struct AgentsmdReader {
    root: PathBuf,
}

impl AgentsmdReader {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();

        Self { root }
    }

    pub fn read(&self) -> std::io::Result<Agentsmd> {
        let file_path = self.root.join(FILENAME);

        let content = std::fs::read_to_string(&file_path)?;

        Ok(Agentsmd::new(file_path, content, true))
    }
}

#[derive(Debug, Clone)]
pub struct Agentsmd {
    pub file_path: PathBuf,
    pub content: String,
    // 文件是否来来源于当前路径的 AGENTS.md 文件
    pub is_root: bool,
}

impl Agentsmd {
    pub fn new(file_path: impl AsRef<Path>, content: impl Into<String>, is_root: bool) -> Self {
        let file_path = file_path.as_ref().to_path_buf();
        let content = content.into();

        Self { file_path, content, is_root }
    }

    pub fn render(&self) -> String {
        if self.is_root {
            format!("以下内容来自 {} (项目指令):\n\n{}", self.file_path.display(), self.content)
        } else {
            format!(
                "以下内容来自 {} (子目录项目指令, 因读取了该目录下的文件而加载:\n\n{})",
                self.file_path.display(),
                self.content
            )
        }
    }
}
