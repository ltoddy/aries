use std::env::consts::OS;
use std::time::{SystemTime, UNIX_EPOCH};

use aries_context::GlobalContext;

pub fn render(gctx: &GlobalContext, model: &str) -> String {
    let today = today();
    let current_dir = gctx.current_dir.display();

    [
        "<env>",
        format!("  Model: {}", model).as_str(),
        format!("  Working directory: {current_dir}").as_str(),
        format!("  Platform: {OS}").as_str(),
        format!("  Today's date: {today}").as_str(),
        "</env>",
    ]
    .join("\n")
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
