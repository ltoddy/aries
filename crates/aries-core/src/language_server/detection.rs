use std::path::Path;

#[derive(Debug, Clone)]
pub struct LspServerInfo {
    pub binary: &'static str,
    pub args: &'static [&'static str],
}

impl LspServerInfo {
    pub fn detect(project_dir: impl AsRef<Path>) -> Option<Self> {
        let project_dir = project_dir.as_ref();
        const MARKERS: [(&str, LspServerInfo); 7] = [
            ("Cargo.toml", LspServerInfo::new("rust-analyzer", &[])),
            ("tsconfig.json", LspServerInfo::new("typescript-language-server", &["--stdio"])),
            ("package.json", LspServerInfo::new("typescript-language-server", &["--stdio"])),
            ("go.mod", LspServerInfo::new("gopls", &[])),
            ("pyproject.toml", LspServerInfo::new("pyright-langserver", &["--stdio"])),
            ("requirements.txt", LspServerInfo::new("pyright-langserver", &["--stdio"])),
            ("setup.py", LspServerInfo::new("pyright-langserver", &["--stdio"])),
        ];

        for (marker, info) in MARKERS.into_iter() {
            if project_dir.join(marker).exists() {
                return Some(info);
            }
        }
        None
    }

    pub fn installed(&self) -> bool {
        std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(self.binary).is_file()))
            .unwrap_or(false)
    }

    const fn new(binary: &'static str, args: &'static [&'static str]) -> Self {
        Self { binary, args }
    }
}
