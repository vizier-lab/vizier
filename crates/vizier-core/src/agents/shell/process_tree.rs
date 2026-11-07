//! Killing a timed-out command's whole process tree (`specs/040-projects-mode/contracts/workspace-shell.md`).
//!
//! The platform-specific parts live here and nowhere else (Principle IV): on Unix the child is
//! started in its own process group and the group is signalled, so grandchildren die with it;
//! on Windows `taskkill /T /F` walks the tree.

use tokio::process::{Child, Command};

/// Put a command in its own process group, so the timeout can signal everything it spawned.
#[cfg(unix)]
pub fn isolate(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
pub fn isolate(_command: &mut Command) {}

/// Stop a child and everything it started, then reap it.
#[cfg(unix)]
pub async fn kill(child: &mut Child) {
    if let Some(pid) = child.id() {
        // The child's process group id is its pid (see `isolate`), so `killpg` reaches
        // grandchildren even after the shell itself exits.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(windows)]
pub async fn kill(child: &mut Child) {
    if let Some(pid) = child.id() {
        let mut taskkill = std::process::Command::new("taskkill");
        taskkill
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        crate::process::no_window_blocking(&mut taskkill);
        let _ = taskkill.status();
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(all(not(unix), not(windows)))]
pub async fn kill(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Make a child die with this process, so a crashed or killed Vizier leaves no language server
/// behind (R9). Linux asks the kernel (`PR_SET_PDEATHSIG`), Windows puts the child in a job object
/// that kills it when the last handle closes, and macOS has no such signal, so a server there
/// relies on exiting when its stdin closes.
#[cfg(target_os = "linux")]
pub fn bind_to_parent(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn bind_to_parent(_command: &mut Command) {}

/// Windows: assign the spawned child to the process-wide kill-on-close job.
#[cfg(windows)]
pub fn bind_to_parent(_command: &mut Command) {}

/// Windows: call after `spawn`, with the child, to join the kill-on-close job object.
#[cfg(windows)]
pub fn bind_child_to_job(child: &Child) {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job as usize
    });
    if job != 0
        && let Some(handle) = child.raw_handle()
    {
        unsafe {
            AssignProcessToJobObject(job as HANDLE, handle as HANDLE);
        }
    }
}

/// Other platforms: nothing to join.
#[cfg(not(windows))]
pub fn bind_child_to_job(_child: &Child) {}

#[cfg(all(not(unix), not(windows)))]
pub fn bind_to_parent(_command: &mut Command) {}
