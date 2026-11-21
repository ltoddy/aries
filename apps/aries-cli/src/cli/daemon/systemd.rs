use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::{env, io, str};

use tokio::process::Command;

use crate::cli::daemon::error::DaemonError;

pub const UNIT: &str = "aries-daemon.service";
const SHOW_ARGS: &[&str] = &[
    "show",
    "--all",
    "--property=LoadState,ActiveState,SubState,MainPID,ExecMainCode,ExecMainStatus,UnitFileState,FragmentPath",
    UNIT,
];
const UNIT_NOT_FOUND: i32 = 4;

pub async fn ensure_service_absent() -> crate::Result<(), DaemonError> {
    let state = query().await?;
    if !state.is_missing() {
        return Err(DaemonError::already_registered(format!(
            "{UNIT} (systemd --user, LoadState={}, ActiveState={}, SubState={})",
            state.load_state, state.active_state, state.sub_state
        )));
    }
    Ok(())
}

pub fn unit_path(home: &Path) -> crate::Result<PathBuf, DaemonError> {
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let path = config.join("systemd/user").join(UNIT);
    validate_path(&path)?;
    Ok(path)
}

pub async fn unit_file_exists(path: &Path) -> crate::Result<bool, DaemonError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(DaemonError::io(
            format!("failed to inspect systemd user unit file {}", path.display()),
            source,
        )),
    }
}

pub fn validate_path(path: &Path) -> crate::Result<&str, DaemonError> {
    if !path.is_absolute() {
        return Err(DaemonError::invalid_path(path, "path must be absolute"));
    }
    let value =
        path.to_str().ok_or_else(|| DaemonError::invalid_path(path, "path is not UTF-8"))?;
    if value.chars().any(char::is_control) {
        return Err(DaemonError::invalid_path(path, "path contains a control character"));
    }
    Ok(value)
}

pub async fn systemctl(args: &[&str], context: &str) -> crate::Result<Output, DaemonError> {
    Command::new("systemctl").arg("--user").args(args).env("LC_ALL", "C").output().await.map_err(
        |source| {
            DaemonError::io(
                format!("failed to execute systemctl --user {}{context}", args.join(" ")),
                source,
            )
        },
    )
}

pub async fn query() -> crate::Result<UnitStatus, DaemonError> {
    let output = systemctl(SHOW_ARGS, "").await?;
    let text = str::from_utf8(&output.stdout);
    let missing = output.status.code() == Some(UNIT_NOT_FOUND)
        && text.as_ref().is_ok_and(|text| {
            let mut load_states = text
                .lines()
                .filter_map(|line| line.split_once('='))
                .filter(|(key, _)| *key == "LoadState")
                .map(|(_, value)| value);
            matches!((load_states.next(), load_states.next()), (Some("not-found"), None))
        });
    if !output.status.success() && !missing {
        return Err(DaemonError::command(
            format!("systemctl --user {}", SHOW_ARGS.join(" ")),
            output,
        ));
    }
    let text = text.map_err(|source| {
        DaemonError::invalid_status(format!("systemctl show returned non-UTF-8 stdout: {source}"))
    })?;
    UnitStatus::new(text)
}

#[derive(Debug)]
pub struct UnitStatus {
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub main_pid: Option<u32>,
    pub exec_main_code: Option<String>,
    pub exec_main_status: Option<String>,
    pub unit_file_state: Option<String>,
    pub fragment_path: Option<String>,
}

impl UnitStatus {
    fn new(output: &str) -> crate::Result<Self, DaemonError> {
        let mut properties = BTreeMap::new();
        for line in output.lines().filter(|line| !line.is_empty()) {
            let (key, value) = line.split_once('=').ok_or_else(|| {
                DaemonError::invalid_status(format!("expected key=value, got {line:?}"))
            })?;
            if key.is_empty() || properties.insert(key, value).is_some() {
                return Err(DaemonError::invalid_status(format!(
                    "empty or duplicate property key in {line:?}"
                )));
            }
        }
        let required = |key| {
            properties
                .get(key)
                .copied()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    DaemonError::invalid_status(format!("missing or empty {key}: {output}"))
                })
        };
        let optional =
            |key| properties.get(key).copied().filter(|value| !value.is_empty()).map(str::to_owned);
        let main_pid = optional("MainPID")
            .map(|value| {
                value.parse::<u32>().map_err(|source| {
                    DaemonError::invalid_status(format!("invalid MainPID {value:?}: {source}"))
                })
            })
            .transpose()?
            .filter(|pid| *pid != 0);
        Ok(Self {
            load_state: required("LoadState")?,
            active_state: required("ActiveState")?,
            sub_state: required("SubState")?,
            main_pid,
            exec_main_code: optional("ExecMainCode"),
            exec_main_status: optional("ExecMainStatus"),
            unit_file_state: optional("UnitFileState"),
            fragment_path: optional("FragmentPath"),
        })
    }

    pub fn is_missing(&self) -> bool {
        self.load_state == "not-found" && self.active_state == "inactive" && self.main_pid.is_none()
    }
}
