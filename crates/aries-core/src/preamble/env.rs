use std::env::consts::OS;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn render(cwd: impl AsRef<Path>, model: &str) -> String {
    let today = today();
    let cwd = cwd.as_ref();
    let is_git_repository = git2::Repository::discover(cwd).is_ok();
    let cwd = cwd.display();

    [
        String::from("<env>"),
        format!("  Model: {}", model),
        format!("  Working directory: {cwd}"),
        format!("  Platform: {OS}"),
        format!("  Today's date: {today}"),
        format!(
            "  Is directory a git repo: Yes/No: {}",
            if is_git_repository { "yes" } else { "no" }
        ),
        String::from("</env>"),
    ]
    .join(",")
}

fn today() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let days = (secs / 86400) as i64;
    let mut y = 1970;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }

    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
