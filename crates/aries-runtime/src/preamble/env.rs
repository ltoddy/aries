use std::env::consts::OS;
use std::path::Path;

pub fn section(cwd: impl AsRef<Path>) -> String {
    let cwd = cwd.as_ref();
    let cwd = cwd.display();

    [
        String::from("<env>"),
        format!("  Working directory: {cwd}"),
        format!("  Platform: {OS}"),
        String::from("</env>"),
    ]
    .join("\n")
}
