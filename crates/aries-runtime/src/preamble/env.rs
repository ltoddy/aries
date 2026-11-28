use std::env::consts::OS;
use std::path::Path;

use crate::sandbox::SandboxMode;

pub fn section(cwd: impl AsRef<Path>, sandbox_mode: SandboxMode) -> String {
    let cwd = cwd.as_ref();

    [
        String::from("<env>"),
        format!("  Working directory: {}", cwd.display()),
        format!("  Platform: {OS}"),
        String::from("</env>"),
        String::new(),
        format!("<sandbox mode=\"{sandbox_mode}\">"),
        String::from(
            "If the sandbox blocks a command, adapt instead of retrying — ask the user to switch mode only if truly necessary.",
        ),
        String::from("</sandbox>"),
    ]
    .join("\n")
}
