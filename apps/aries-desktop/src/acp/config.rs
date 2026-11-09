use std::path::PathBuf;

use agent_client_protocol::AcpAgentConfig;

pub fn aries_config() -> AcpAgentConfig {
    let executable = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|path| path.join(if cfg!(windows) { "aries.exe" } else { "aries" }))
        .find(|path| path.is_file())
        .or_else(|| {
            dirs::home_dir().map(|home| home.join(".cargo/bin/aries")).filter(|path| path.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("aries"));
    AcpAgentConfig::new(executable).arg("acp")
}
