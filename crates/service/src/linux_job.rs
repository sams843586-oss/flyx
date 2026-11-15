//! systemd KillMode=control-group covers descendants; PDEATHSIG also covers
//! direct core children if the supervisor is killed outside systemd.
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

pub struct Job;
impl Job {
    pub fn new() -> io::Result<Self> { Ok(Self) }
    pub fn assign(&self, _: &Child) -> io::Result<()> { Ok(()) }
}

pub fn prepare(command: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(io::Error::other("VPN supervisor exited before core startup"));
            }
            Ok(())
        });
    }
}
