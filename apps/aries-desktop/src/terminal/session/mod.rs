use std::collections::VecDeque;
#[cfg(unix)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions as GridDimensions;
use alacritty_terminal::term::{Config, Osc52};
#[cfg(unix)]
use alacritty_terminal::vte::ansi::Processor;
#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    Exited { code: u32, signal: Option<String> },
    Failed(String),
}

impl Status {
    pub fn starting() -> Self {
        Self::Starting
    }
    pub fn running() -> Self {
        Self::Running
    }
    pub fn exited(code: u32, signal: Option<String>) -> Self {
        Self::Exited { code, signal }
    }
    pub fn failed(error: impl std::fmt::Display) -> Self {
        Self::Failed(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub columns: usize,
    pub lines: usize,
}

impl Dimensions {
    pub fn new(columns: usize, lines: usize) -> Self {
        Self { columns, lines }
    }
}

impl GridDimensions for Dimensions {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "终端尺寸超出允许范围：{columns} 列、{lines} 行（列数须为 2–1000，行数须为 1–1000）",
    columns = .dimensions.columns,
    lines = .dimensions.lines
)]
pub struct InvalidDimensions {
    pub dimensions: Dimensions,
}

impl InvalidDimensions {
    pub fn new(dimensions: Dimensions) -> Self {
        Self { dimensions }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("terminal {resource} lock poisoned")]
pub struct LockError {
    resource: &'static str,
}

impl LockError {
    pub fn new(resource: &'static str) -> Self {
        Self { resource }
    }
}

fn lock<'a, T>(
    mutex: &'a Mutex<T>,
    resource: &'static str,
) -> Result<MutexGuard<'a, T>, LockError> {
    mutex.lock().map_err(|_| LockError::new(resource))
}

mod error;
mod runtime;
mod worker;

pub use self::error::{StartError, WriteError};
pub use self::runtime::Session;

fn validate_dimensions(dimensions: Dimensions) -> Result<(), InvalidDimensions> {
    if !(2..=1000).contains(&dimensions.columns) || !(1..=1000).contains(&dimensions.lines) {
        return Err(InvalidDimensions::new(dimensions));
    }
    Ok(())
}

#[cfg(unix)]
fn shell() -> PathBuf {
    if let Some(value) = std::env::var_os("SHELL").filter(|value| !value.is_empty()) {
        return value.into();
    }
    #[cfg(unix)]
    unsafe {
        let mut storage = vec![0u8; 16384];
        let mut record: libc::passwd = std::mem::zeroed();
        let mut entry = std::ptr::null_mut();
        let result = libc::getpwuid_r(
            libc::getuid(),
            &mut record,
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut entry,
        );
        if result == 0 && !entry.is_null() && !(*entry).pw_shell.is_null() {
            let value = std::ffi::CStr::from_ptr((*entry).pw_shell).to_string_lossy();
            if !value.is_empty() {
                return PathBuf::from(value.as_ref());
            }
        }
    }
    PathBuf::from("/bin/sh")
}

#[cfg(unix)]
fn signal_group(pid: i32, group: i32, session: i32, signal: i32) {
    if group > 1
        && group != unsafe { libc::getpgrp() }
        && unsafe { libc::getsid(pid) } == session
        && unsafe { libc::getpgid(pid) } == group
    {
        unsafe {
            libc::kill(-group, signal);
        }
    }
}

#[cfg(target_os = "macos")]
fn process_alive(pid: i32) -> bool {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        ) == size
            && info.pbi_status != libc::SZOMB
    }
}

#[cfg(target_os = "linux")]
fn process_alive(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|stat| {
        stat.rsplit_once(')').map(|(_, fields)| fields.trim_start().starts_with(['Z', 'X']))
    }) == Some(false)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn process_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(unix)]
fn session_processes(session: i32, deadline: Instant) -> Vec<(i32, i32)> {
    #[cfg(target_os = "macos")]
    let pids = {
        let bytes = unsafe { libc::proc_listpids(1, 0, std::ptr::null_mut(), 0) };
        let mut pids = vec![0i32; (bytes.max(0) as usize / std::mem::size_of::<i32>()) + 1024];
        let bytes = unsafe {
            libc::proc_listpids(
                1,
                0,
                pids.as_mut_ptr().cast(),
                (pids.len() * std::mem::size_of::<i32>()) as i32,
            )
        };
        pids.truncate(bytes.max(0) as usize / std::mem::size_of::<i32>());
        pids
    };
    #[cfg(target_os = "linux")]
    let pids = std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<i32>().ok());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let pids = [session];
    let mut processes = Vec::new();
    for pid in pids {
        if Instant::now() >= deadline {
            break;
        }
        if pid > 1 && unsafe { libc::getsid(pid) } == session && process_alive(pid) {
            let group = unsafe { libc::getpgid(pid) };
            if group > 1 {
                processes.push((pid, group));
            }
        }
    }
    processes
}

#[cfg(unix)]
fn child_exited(pid: u32) -> std::io::Result<bool> {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(libc::P_PID, pid, &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.si_pid() } != 0)
}
