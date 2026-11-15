//! Privileged VPN supervisor: Windows service or Linux systemd daemon.
#[cfg(windows)] mod job;
#[cfg(windows)] mod pipe;
#[cfg(windows)] mod security;
#[cfg(windows)] mod windows;
#[cfg(target_os = "linux")] #[path = "linux_job.rs"] mod job;
#[cfg(target_os = "linux")] #[path = "unix_pipe.rs"] mod pipe;
#[cfg(target_os = "linux")] mod linux;
mod supervisor;

fn main() {
    #[cfg(windows)] windows::run();
    #[cfg(target_os = "linux")] linux::run();
}
