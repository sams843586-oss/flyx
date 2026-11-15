use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use flexconnect_core::ipc::Request;
use crate::supervisor::{Env, Supervisor};

/// Never run user-writable binaries as root or store root configs in a shared dir.
pub fn check_root_owned(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    let correct_type = if directory { metadata.is_dir() } else { metadata.is_file() };
    if !correct_type || metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            format!("{} must be root-owned and not writable by other users", path.display())));
    }
    Ok(())
}

fn serve() -> Result<(), Box<dyn std::error::Error>> {
    if unsafe { libc::geteuid() } != 0 { return Err("start the installed service with sudo systemctl start flyx".into()); }
    // New files start private; the IPC socket is explicitly made group-accessible.
    unsafe { libc::umask(0o077); }
    let runtime = Path::new("/run/flexconnect");
    check_root_owned(runtime, true)?;
    let bin_dir = PathBuf::from("/usr/lib/flyx");
    check_root_owned(&bin_dir, true)?;
    let run_dir = runtime.join("config");
    flexconnect_core::platform::private_dir(&run_dir)?;
    let supervisor = Arc::new(Mutex::new(Supervisor::new(Env { bin_dir, run_dir })?));
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = stop.clone();
    ctrlc::set_handler(move || { signal_stop.store(true, Ordering::SeqCst); })?;
    let (watch_stop, watch_supervisor) = (stop.clone(), supervisor.clone());
    let watcher = std::thread::spawn(move || {
        while !watch_stop.load(Ordering::SeqCst) {
            watch_supervisor.lock().unwrap().handle(Request::Status);
            std::thread::sleep(Duration::from_millis(250));
        }
    });
    let shared = supervisor.clone();
    let result = crate::pipe::serve(stop.clone(), move |line| {
        let mut supervisor = shared.lock().unwrap();
        let response = match serde_json::from_str::<Request>(line) {
            Ok(request) => supervisor.handle(request),
            Err(e) => supervisor.bad_request(format!("invalid request: {e}")),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".into())
    });
    stop.store(true, Ordering::SeqCst);
    watcher.join().ok();
    supervisor.lock().unwrap().stop();
    Ok(result?)
}

pub fn run() {
    let command = std::env::args().nth(1);
    if command.as_deref().is_some_and(|s| s != "run") {
        eprintln!("Linux: install the package, then use systemctl start/stop flyx");
        std::process::exit(2);
    }
    if let Err(error) = serve() {
        eprintln!("Flyx service: {error}");
        std::process::exit(1);
    }
}
