use std::path::Path;

#[derive(Debug, Clone)]
pub struct LspServerInfo {
    pub name: &'static str,
    pub binary: &'static str,
}

pub fn detect_language_server(project_dir: &Path) -> Option<LspServerInfo> {
    let markers: &[(&str, LspServerInfo)] = &[
        ("Cargo.toml", LspServerInfo { name: "Rust Analyzer", binary: "rust-analyzer" }),
        (
            "tsconfig.json",
            LspServerInfo {
                name: "TypeScript Language Server",
                binary: "typescript-language-server",
            },
        ),
        (
            "package.json",
            LspServerInfo {
                name: "TypeScript Language Server",
                binary: "typescript-language-server",
            },
        ),
        ("go.mod", LspServerInfo { name: "gopls", binary: "gopls" }),
        ("pyproject.toml", LspServerInfo { name: "Pyright", binary: "pyright-langserver" }),
        ("requirements.txt", LspServerInfo { name: "Pyright", binary: "pyright-langserver" }),
        ("setup.py", LspServerInfo { name: "Pyright", binary: "pyright-langserver" }),
    ];

    for (marker, info) in markers {
        if project_dir.join(marker).exists() {
            return Some(info.clone());
        }
    }
    None
}

pub fn is_binary_installed(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(binary).is_file()))
        .unwrap_or(false)
}
