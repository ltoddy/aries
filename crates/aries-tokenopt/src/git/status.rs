use std::process::Command;

use itertools::Itertools;

use crate::Output;
use crate::git::GitError;

pub fn execute(args: Vec<String>, rest_args: Vec<String>) -> Result<Output, GitError> {
    let rest_args: Vec<&str> = rest_args.iter().map(String::as_str).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let output = run_git_status(&rest_args, &args)?;
    if !is_compact_status(&args) {
        return Ok(Output::stdout(filter_hints(&output)));
    }

    let porcelain = run_git_status(&rest_args, &["--porcelain", "-b"])?;
    let detached = find_detached_line(&output);
    let body = format_body(&porcelain, detached.as_deref());
    let stdout = extract_state_header(&output).map(|h| format!("{h}\n{body}")).unwrap_or(body);

    Ok(Output { stdout, stderr: String::new(), exit_code: 0 })
}

fn run_git_status(rest_args: &[&str], extra_args: &[&str]) -> Result<String, GitError> {
    let output = make_status_command(rest_args, extra_args).output()?;

    if String::from_utf8_lossy(&output.stderr).contains("not a git repository") {
        return Err(GitError::NotARepo);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn is_compact_status(args: &[&str]) -> bool {
    if args.is_empty() {
        return true;
    }
    let mut has_branch = false;
    for a in args {
        match *a {
            "-b" | "--branch" | "-sb" | "-bs" => has_branch = true,
            "-s" | "--short" => {},
            _ => return false,
        }
    }
    has_branch
}

fn filter_hints(output: impl Into<String>) -> String {
    let output = output.into();

    fn is_noise(line: &str) -> bool {
        let line = line.trim();
        line.is_empty()
            || line.starts_with(r#"(use "git"#)
            || line.starts_with("(create/copy files")
            || line.contains(r#"(use "git add"#)
            || line.contains(r#"(use "git restore"#)
    }

    output.lines().filter(|line| !is_noise(line)).join("\n")
}

fn make_status_command(rest_args: &[&str], extra_args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("status").args(rest_args).args(extra_args).env("LC_ALL", "C");
    cmd
}

fn format_body(porcelain: impl Into<String>, detached: Option<&str>) -> String {
    let porcelain = porcelain.into();
    let lines = porcelain.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>();

    let (first, rest) = match lines.split_first() {
        None => return "Clean working tree".to_string(),
        Some(split) => split,
    };

    let is_branch = first.starts_with("##");
    let mut output = Vec::with_capacity(lines.len() + 1);

    output.push(if is_branch {
        let branch = first.trim_start_matches("## ");
        format!("* {}", detached.unwrap_or(branch))
    } else {
        first.to_string()
    });

    output.extend(rest.iter().copied().map(str::to_string));

    if rest.is_empty() && is_branch {
        output.push("clean — nothing to commit".to_string());
    }

    output.join("\n")
}

fn extract_state_header(raw: impl Into<String>) -> Option<String> {
    let raw = raw.into();

    const STOPPERS: &[&str] = &[
        "Changes to be committed:",
        "Changes not staged for commit:",
        "Untracked files:",
        "Unmerged paths:",
        "no changes added to commit",
        "nothing to commit",
        "nothing added to commit",
    ];

    raw.lines()
        .map(str::trim)
        .take_while(|s| !STOPPERS.iter().any(|p| s.starts_with(p)))
        .find_map(|s| GitStatusState::from_line(s).map(|state| state.summary().to_string()))
}

fn find_detached_line(raw: &str) -> Option<String> {
    raw.lines().map(str::trim).find(|l| l.starts_with("HEAD detached ")).map(str::to_string)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitStatusState {
    Rebase,
    MergeConflicts,
    MergeReadyToCommit,
    CherryPick,
    Revert,
    Bisect,
    Am,
    SparseCheckout,
}

impl GitStatusState {
    fn summary(self) -> &'static str {
        match self {
            Self::Rebase => "rebase in progress",
            Self::MergeConflicts => "merge in progress. unresolved conflicts",
            Self::MergeReadyToCommit => "merge in progress. no conflicts",
            Self::CherryPick => "cherry-pick in progress",
            Self::Revert => "revert in progress",
            Self::Bisect => "bisect in progress",
            Self::Am => "am session in progress",
            Self::SparseCheckout => "sparse checkout enabled",
        }
    }

    fn from_line(line: &str) -> Option<Self> {
        let patterns: &[(&str, Self)] = &[
            ("All conflicts fixed but you are still merging", Self::MergeReadyToCommit),
            ("You have unmerged paths", Self::MergeConflicts),
            ("You are currently cherry-picking", Self::CherryPick),
            ("You are currently reverting", Self::Revert),
            ("You are currently bisecting", Self::Bisect),
            ("You are in the middle of an am session", Self::Am),
            ("You are in a sparse checkout", Self::SparseCheckout),
        ];

        let rebase_indicators: &[&str] = &[
            "rebase in progress",
            "You are currently rebasing",
            "You are currently editing",
            "You are currently splitting",
            "Last command done",
            "Next command to do",
            "No commands remaining",
        ];

        patterns
            .iter()
            .find(|(p, _)| line.contains(p))
            .map(|(_, state)| *state)
            .or_else(|| rebase_indicators.iter().any(|i| line.contains(i)).then_some(Self::Rebase))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_clean() {
        let result = format_body("## main...origin/main\n", None);
        assert_eq!(result, "* main...origin/main\nclean — nothing to commit");
    }

    #[test]
    fn format_with_changes() {
        let porcelain =
            "## main...origin/main\nM  src/main.rs\n M src/core/utils.rs\n?? new_file.txt\n";
        let result = format_body(porcelain, None);
        assert_eq!(
            result,
            "* main...origin/main\nM  src/main.rs\n M src/core/utils.rs\n?? new_file.txt"
        );
    }

    #[test]
    fn format_empty() {
        assert_eq!(format_body("", None), "Clean working tree");
    }

    #[test]
    fn format_detached() {
        let porcelain = "## HEAD (no branch)\nM  file.rs\n";
        let result = format_body(porcelain, Some("abc1234 (HEAD detached)"));
        assert_eq!(result, "* abc1234 (HEAD detached)\nM  file.rs");
    }

    #[test]
    fn filter_removes_hints() {
        let raw = "\
On branch main
Your branch is up to date with 'origin/main'.

Changes to be committed:
  (use \"git restore --staged <file>...\" to unstage)
        modified:   src/main.rs

Changes not staged for commit:
  (use \"git add <file>...\" to update what will be committed)
  (use \"git restore <file>...\" to discard changes in working directory)
        modified:   src/core/utils.rs

Untracked files:
  (use \"git add <file>...\" to include in what will be committed)
        new_file.txt
";
        let result = filter_hints(raw);
        assert!(!result.contains(r#"(use "git"#));
        assert!(result.contains("On branch main"));
        assert!(result.contains("modified:   src/main.rs"));
        assert!(result.contains("modified:   src/core/utils.rs"));
        assert!(result.contains("new_file.txt"));
    }

    #[test]
    fn filter_clean_tree() {
        let raw = "\
On branch main
Your branch is up to date with 'origin/main'.

nothing to commit, working tree clean
";
        let result = filter_hints(raw);
        assert!(result.contains("nothing to commit, working tree clean"));
    }

    #[test]
    fn extract_rebase_state() {
        let raw = "\
interactive rebase in progress; onto abc1234
Last command done (1 command done):
   pick abc1234 some commit
No commands remaining.
Changes to be committed:
        modified:   file.rs
";
        assert_eq!(extract_state_header(raw), Some("rebase in progress".to_string()));
    }

    #[test]
    fn extract_merge_conflicts_state() {
        let raw = "\
On branch main
You have unmerged paths.
  (fix conflicts and run \"git commit\")

Unmerged paths:
        both modified:   file.rs
";
        assert_eq!(
            extract_state_header(raw),
            Some("merge in progress. unresolved conflicts".to_string())
        );
    }

    #[test]
    fn extract_detached() {
        let raw = "HEAD detached at abc1234\nnothing to commit, working tree clean\n";
        assert_eq!(find_detached_line(raw), Some("HEAD detached at abc1234".to_string()));
    }

    #[test]
    fn extract_detached_on_branch() {
        let raw = "On branch main\nnothing to commit, working tree clean\n";
        assert_eq!(find_detached_line(raw), None);
    }

    #[test]
    fn compact_status_path() {
        assert!(is_compact_status(&[]));
        assert!(is_compact_status(&["-b"]));
        assert!(is_compact_status(&["--branch"]));
        assert!(is_compact_status(&["-sb"]));
        assert!(!is_compact_status(&["--", "file.rs"]));
        assert!(!is_compact_status(&["-u"]));
    }
}
