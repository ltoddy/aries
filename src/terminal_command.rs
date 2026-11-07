use std::cell::Cell;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{Context, bail};
use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use crate::Quit;

const COMMAND_PATH: &str = "/usr/local/bin/aries-orbit";

gpui_kit::actions!(orbit, [InstallTerminalCommand]);

pub fn init(cx: &mut App) {
    let installing = Rc::new(Cell::new(false));
    cx.on_action(move |_: &InstallTerminalCommand, cx| {
        if installing.get() {
            return;
        }
        let Some(window) = cx.active_window() else {
            return;
        };
        installing.set(true);
        let installing = installing.clone();
        cx.spawn(async move |cx| {
            let result = cx.background_executor().spawn(async move { install() }).await;
            installing.set(false);
            let _ = window.update(cx, |_, window, cx| {
                let notification = match result {
                    Ok(()) => Notification::success(
                        "已安装终端命令。在终端输入 aries-orbit 即可启动应用。",
                    ),
                    Err(error) => Notification::error(format!("无法安装终端命令：{error:#}")),
                };
                window.push_notification(notification.autohide(false), cx);
            });
        })
        .detach();
    });
    cx.set_menus([Menu::new("Aries Orbit").items([
        MenuItem::action("安装终端命令…", InstallTerminalCommand),
        MenuItem::separator(),
        MenuItem::action("退出 Aries Orbit", Quit),
    ])]);
}

fn install() -> anyhow::Result<()> {
    let executable = env::current_exe().context("无法定位应用")?.canonicalize()?;
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .context("无法定位应用安装目录")?;
    let user_applications = dirs::home_dir().map(|home| home.join("Applications"));
    if bundle.extension().is_none_or(|extension| extension != "app")
        || !(bundle.starts_with("/Applications")
            || user_applications.as_ref().is_some_and(|directory| bundle.starts_with(directory)))
    {
        bail!("请先将 Aries Orbit.app 安装到 Applications 文件夹，再从已安装的应用重试。")
    }
    let launcher = bundle.join("Contents/Resources/bin/aries-orbit");
    if !launcher.is_file() {
        bail!("应用缺少终端启动器，请重新安装应用。")
    }
    let destination = PathBuf::from(COMMAND_PATH);
    let script = installation_script(&launcher, &destination)?;
    let applescript =
        format!("do shell script {} with administrator privileges", applescript_string(&script));
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &applescript])
        .output()
        .context("无法请求系统安装授权")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

fn installation_script(launcher: &Path, destination: &Path) -> anyhow::Result<String> {
    let launcher = shell_string(launcher.to_str().context("应用路径不是有效的 UTF-8")?);
    let directory =
        shell_string(destination.parent().and_then(Path::to_str).context("无效的命令安装目录")?);
    let destination = shell_string(destination.to_str().context("无效的命令安装路径")?);
    Ok(format!(
        "set -eu; if [ -L {destination} ] && [ \"$(/usr/bin/readlink {destination})\" = {launcher} ]; then exit 0; fi; if [ -e {destination} ] || [ -L {destination} ]; then printf '%s\\n' '安装位置已有同名命令，请先移除或重命名后重试。' >&2; exit 1; fi; /bin/mkdir -p {directory}; /bin/ln -s {launcher} {destination}"
    ))
}

fn shell_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::terminal_command::{applescript_string, installation_script, shell_string};

    #[test]
    fn quotes_shell_values_without_expansion() {
        let value = "a 'quoted' \"path\" $HOME `id` \\ folder";
        let output = Command::new("/bin/sh")
            .args(["-c", &format!("printf '%s' {}", shell_string(value))])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
    }

    #[test]
    fn quotes_applescript_strings() {
        assert_eq!(applescript_string("a\\b\"c"), "\"a\\\\b\\\"c\"");
    }

    #[test]
    fn installs_idempotently_and_preserves_conflicting_commands() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let directory =
            std::env::temp_dir().join(format!("aries-orbit-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let launcher = directory.join("Aries Orbit's launcher");
        fs::write(&launcher, "launcher").unwrap();
        let destination = directory.join("bin/aries-orbit");
        let script = installation_script(&launcher, &destination).unwrap();
        for _ in 0..2 {
            let output = Command::new("/bin/sh").args(["-c", &script]).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(fs::read_link(&destination).unwrap(), launcher);
        }
        fs::remove_file(&destination).unwrap();
        fs::write(&destination, "existing command").unwrap();
        let output = Command::new("/bin/sh").args(["-c", &script]).output().unwrap();
        assert!(!output.status.success());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "existing command");
        fs::remove_file(&destination).unwrap();
        std::os::unix::fs::symlink(directory.join("missing"), &destination).unwrap();
        let output = Command::new("/bin/sh").args(["-c", &script]).output().unwrap();
        assert!(!output.status.success());
        assert_eq!(fs::read_link(&destination).unwrap(), directory.join("missing"));
        fs::remove_dir_all(&directory).unwrap();
    }
}
