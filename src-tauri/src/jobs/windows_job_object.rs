// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows Job Object process-tree control (architecture.md §10.10;
//! design-02 §2.2, §2.5 step 4). Compiled and `cargo check`/`clippy`/
//! `cargo test`-verified on this development machine.
//!
//! `pgn-extract` spawns no children of its own, so the "assign after
//! start" window (the child briefly exists before it is assigned to the
//! job object) is acceptable, exactly as design-02 notes - this is
//! defense-in-depth and the documented `TerminateJobObject` mechanism for
//! cancellation, not a requirement for correctness of a single-process
//! engine invocation.

use std::io;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub(crate) struct JobObject {
    handle: HANDLE,
}

// SAFETY: `HANDLE` (a Win32 kernel handle) is not thread-affine; every use
// of `self.handle` here is a single, self-contained WinAPI call performed
// through `&self`/`&mut self` methods, never concurrent raw access.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    /// Creates an anonymous job object configured with
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so closing the last handle to
    /// it (e.g. the whole app process dying) kills every process still
    /// assigned to it.
    pub fn create() -> io::Result<Self> {
        // SAFETY: null security attributes (default security) and a null
        // name (anonymous job object) are both documented-valid arguments
        // to `CreateJobObjectW`.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = JobObject { handle };

        // SAFETY: `info` is a valid, fully-initialized
        // `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` (zeroed, which is a valid
        // bit pattern for every field - all integers/pointer-sized, no
        // non-nullable invariants - with exactly one field then set); its
        // address and exact size are passed to `SetInformationJobObject`.
        let ok = unsafe {
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Assigns a child process to this job object so that
    /// [`JobObject::terminate`] kills it (and any descendants it might
    /// ever spawn, even though `pgn-extract` does not spawn any today).
    pub fn assign(&self, process_handle: HANDLE) -> io::Result<()> {
        // SAFETY: `self.handle` is a valid job object handle for the
        // lifetime of `self`; `process_handle` is the caller's
        // responsibility (it must be a valid, still-open process handle -
        // `jobs::process` calls this immediately after spawn, before the
        // child handle is used for anything else).
        let ok = unsafe { AssignProcessToJobObject(self.handle, process_handle) };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// `TerminateJobObject` - kills every process in the job atomically
    /// (design-02 §2.5 step 4: "kills the entire tree atomically").
    pub fn terminate(&self) -> io::Result<()> {
        // SAFETY: `self.handle` is a valid job object handle for the
        // lifetime of `self`.
        let ok = unsafe { TerminateJobObject(self.handle, 1) };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is a valid, owned handle, not read again
        // after this call.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_succeeds_and_closes_cleanly() {
        let job = JobObject::create().unwrap();
        drop(job);
    }

    #[test]
    fn assign_and_terminate_a_real_short_lived_process() {
        use std::os::windows::io::AsRawHandle;
        // `ping` is spawned directly (argv array, no shell - consistent
        // with this codebase's own no-shell rule even in test fixtures)
        // as an inert, long-enough-lived decoy process to exercise
        // assign+terminate; it is not anything the app ships or depends
        // on. Unlike `timeout.exe`, `ping.exe` does not reject redirected/
        // null stdio.
        let mut child = std::process::Command::new("ping")
            .args(["-n", "31", "127.0.0.1"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("ping.exe must be available to spawn a decoy process");
        let raw = child.as_raw_handle() as HANDLE;

        let job = JobObject::create().unwrap();
        job.assign(raw)
            .expect("assign must succeed for a live process");
        job.terminate().expect("terminate must succeed");

        let status = child.wait().expect("terminated child must be waitable");
        assert!(
            !status.success(),
            "a terminated process must not report success"
        );
    }
}
