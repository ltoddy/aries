//! Cross-platform shell detection and command construction.
//!
//! On Unix we honor `$SHELL` (falling back to `/bin/sh`); on Windows we look for
//! `pwsh.exe` (PowerShell 7+) -> `powershell.exe` (Windows PowerShell 5.1) ->
//! `%COMSPEC%` (cmd.exe). The user may override detection with the
//! `ARIES_SHELL` environment variable, which accepts either a bare name like
//! `pwsh` / `bash` or an absolute path.

use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Identifies the family of shell we are driving so callers can branch on
/// syntax-specific optimizations (e.g. tree-sitter-bash rewriting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    PowerShell,
    Cmd,
}

/// A fully resolved shell invocation template: the program to launch plus the
/// fixed leading arguments (e.g. `["-c"]` for bash, `["-NoProfile", "-NoLogo",
/// "-Command"]` for PowerShell). The command string is appended as the final
/// argument by [`ShellSpec::build_command`].
#[derive(Debug, Clone)]
pub struct ShellSpec {
    program: PathBuf,
    args: &'static [&'static str],
    pub kind: ShellKind,
}

impl ShellSpec {
    fn new(program: PathBuf, args: &'static [&'static str], kind: ShellKind) -> Self {
        Self { program, args, kind }
    }

    /// Constructs a `tokio::process::Command` that runs `command` through this
    /// shell, with `cwd` set as the working directory. The caller is expected
    /// to configure stdio / kill_on_drop / etc. afterwards.
    pub fn build_command(&self, command: &str, cwd: &Path) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(self.args).arg(command).current_dir(cwd);
        cmd
    }
}

impl ShellSpec {
    fn bash(program: PathBuf) -> Self {
        Self::new(program, &["-c"], ShellKind::Bash)
    }

    fn powershell(program: PathBuf) -> Self {
        Self::new(program, &["-NoProfile", "-NoLogo", "-Command"], ShellKind::PowerShell)
    }

    fn cmd(program: PathBuf) -> Self {
        Self::new(program, &["/C"], ShellKind::Cmd)
    }
}

/// Detect the shell to use for executing commands.
///
/// Priority:
/// 1. `ARIES_SHELL` environment variable (bare name or absolute path).
/// 2. On Windows: `pwsh.exe` -> `powershell.exe` -> `%COMSPEC%` (cmd.exe).
/// 3. On Unix: `$SHELL` -> `/bin/sh`.
pub fn detect_shell() -> ShellSpec {
    // Read the environment exactly once and delegate to the testable inner
    // helper. Keeping `detect_shell` as a thin wrapper means the override
    // resolution path can be tested without mutating the process environment
    // (which is racy under cargo's default parallel test runner).
    detect_shell_inner(std::env::var("ARIES_SHELL").ok())
}

/// Same as [`detect_shell`] but takes the `ARIES_SHELL` override explicitly,
/// so unit tests can exercise the override flow without `std::env::set_var`
/// (unsafe + non-thread-safe under parallel test execution).
fn detect_shell_inner(env_override: Option<String>) -> ShellSpec {
    if let Some(raw) = env_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return parse_override(trimmed);
        }
    }

    #[cfg(windows)]
    {
        if let Some(p) = which_in_path("pwsh.exe") {
            return ShellSpec::powershell(p);
        }
        if let Some(p) = which_in_path("powershell.exe") {
            return ShellSpec::powershell(p);
        }
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned());
        ShellSpec::cmd(PathBuf::from(comspec))
    }

    #[cfg(not(windows))]
    {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
        ShellSpec::bash(PathBuf::from(sh))
    }
}

fn parse_override(raw: &str) -> ShellSpec {
    // Absolute or relative path with a separator: honor it verbatim.
    let path = Path::new(raw);
    if path.is_file() {
        return classify_by_path(path);
    }

    // Otherwise treat `raw` as a bare name and look it up on PATH.
    let name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| raw.to_string());

    let candidate = bare_name_to_executable(&name);
    if let Some(p) = candidate {
        return classify_by_path(&p);
    }

    // Fallback: if we can't resolve the override, pretend it's the literal
    // program path so the user gets a real "not found" error rather than a
    // silent platform default.
    classify_by_path(path)
}

fn classify_by_path(path: &Path) -> ShellSpec {
    let file =
        path.file_name().map(|f| f.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let basename = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(&file)
        .to_string();

    let program = if cfg!(windows) && path.file_name().is_some() && path.parent().is_none() {
        // Bare name on Windows: keep as-is, rely on CreateProcessW resolution.
        PathBuf::from(basename.clone() + ".exe")
    } else {
        path.to_path_buf()
    };

    match basename.as_str() {
        "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish" => ShellSpec::bash(program),
        "pwsh" | "powershell" => ShellSpec::powershell(program),
        "cmd" => ShellSpec::cmd(program),
        _ => {
            // Unknown shell: default to bash-style `-c` invocation. This is a
            // best-effort choice; callers that need precise semantics should
            // set ARIES_SHELL to one of the recognized names.
            ShellSpec::bash(program)
        },
    }
}

#[cfg(windows)]
fn bare_name_to_executable(name: &str) -> Option<PathBuf> {
    // Search PATH for the executable, appending `.exe` if the user didn't.
    let needle = if name.ends_with(".exe") || name.ends_with(".EXE") {
        name.to_string()
    } else {
        format!("{name}.exe")
    };
    which_in_path(&needle)
}

#[cfg(not(windows))]
fn bare_name_to_executable(name: &str) -> Option<PathBuf> {
    which_in_path(name)
}

/// Minimal `which(1)` implementation: walks `PATH` entries and returns the
/// first matching executable. Returns `None` if not found or if `PATH` is not
/// set. This avoids pulling in a third-party `which` crate.
fn which_in_path(name: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(canonicalize_if_possible(candidate));
        }
    }
    None
}

fn canonicalize_if_possible(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_bash_variants() {
        assert_eq!(classify_by_path(Path::new("/bin/bash")).kind, ShellKind::Bash);
        assert_eq!(classify_by_path(Path::new("/usr/bin/zsh")).kind, ShellKind::Bash);
        assert_eq!(classify_by_path(Path::new("sh")).kind, ShellKind::Bash);
    }

    #[test]
    fn classify_powershell_variants() {
        assert_eq!(classify_by_path(Path::new("pwsh")).kind, ShellKind::PowerShell);
        assert_eq!(classify_by_path(Path::new("powershell")).kind, ShellKind::PowerShell);
        assert_eq!(classify_by_path(Path::new("pwsh.exe")).kind, ShellKind::PowerShell);
    }

    #[cfg(windows)]
    #[test]
    fn classify_cmd() {
        assert_eq!(classify_by_path(Path::new("cmd")).kind, ShellKind::Cmd);
        assert_eq!(classify_by_path(Path::new("cmd.exe")).kind, ShellKind::Cmd);
    }

    #[test]
    fn detect_shell_runs_without_panic() {
        // Snapshot the chosen shell; we only assert it produces a usable
        // program path, since the exact value depends on the host.
        let spec = detect_shell();
        assert!(!spec.program.as_os_str().is_empty());
    }

    #[test]
    fn override_bash_via_inner() {
        // Drive the override path directly so we never touch the process
        // environment. Mutating `ARIES_SHELL` here would race with other
        // tests (e.g. the bash execution tests on Windows) under cargo's
        // parallel test runner.
        let spec = detect_shell_inner(Some("bash".into()));
        assert_eq!(spec.kind, ShellKind::Bash);
    }

    #[test]
    fn override_powershell_via_inner() {
        let spec = detect_shell_inner(Some("pwsh".into()));
        assert_eq!(spec.kind, ShellKind::PowerShell);
    }

    #[test]
    fn override_trims_surrounding_whitespace() {
        let spec = detect_shell_inner(Some("   bash   ".into()));
        assert_eq!(spec.kind, ShellKind::Bash);

        let blank = detect_shell_inner(Some("   ".into()));
        // All-whitespace override is treated as absent: we fall through to the
        // platform default. Just assert it doesn't panic and yields a path.
        assert!(!blank.program.as_os_str().is_empty());
    }

    #[test]
    fn override_none_falls_through() {
        let spec = detect_shell_inner(None);
        assert!(!spec.program.as_os_str().is_empty());
    }
}
