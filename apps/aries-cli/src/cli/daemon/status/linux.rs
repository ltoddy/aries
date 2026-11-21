use aries_init::GlobalContext;

use crate::cli::daemon::error::DaemonError;
use crate::cli::daemon::systemd::{UNIT, query, unit_file_exists, unit_path};

pub async fn execute(gctx: GlobalContext) -> crate::Result<(), DaemonError> {
    let path = unit_path(gctx.home_dir())?;
    let state = query().await?;
    let configured = unit_file_exists(&path).await?;
    if state.is_missing() {
        if configured {
            println!(
                "Status: unit file exists but is not loaded; systemctl --user daemon-reload may be needed"
            );
        } else {
            println!("Status: not installed (unit not found)");
        }
    } else {
        println!("Status: {} ({})", state.active_state, state.sub_state);
    }
    println!("Service: {UNIT} (systemd --user)");
    println!("LoadState: {}", state.load_state);
    println!("ActiveState: {}", state.active_state);
    println!("SubState: {}", state.sub_state);
    if let Some(pid) = state.main_pid {
        println!("PID: {pid}");
    }
    println!("ExecMainCode: {}", state.exec_main_code.as_deref().unwrap_or("(unavailable)"));
    println!("ExecMainStatus: {}", state.exec_main_status.as_deref().unwrap_or("(unavailable)"));
    println!("UnitFileState: {}", state.unit_file_state.as_deref().unwrap_or("(unavailable)"));
    println!("FragmentPath: {}", state.fragment_path.as_deref().unwrap_or("(none)"));
    println!(
        "User unit file: {} ({})",
        path.display(),
        if configured { "present" } else { "absent" }
    );
    println!("Logs: journalctl --user -u {UNIT} -f");
    Ok(())
}
