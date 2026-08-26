use std::env::consts::OS;
use std::path::Path;

use jiff::Zoned;

pub fn section(cwd: impl AsRef<Path>, model: impl Into<String>) -> String {
    let cwd = cwd.as_ref();
    let cwd = cwd.display();
    let now = Zoned::now();

    [
        String::from("<env>"),
        format!("  Model: {}", model.into()),
        format!("  Working directory: {cwd}"),
        format!("  Platform: {OS}"),
        format!("  Today's date: {}", now.date()),
        String::from("</env>"),
    ]
    .join("\n")
}
