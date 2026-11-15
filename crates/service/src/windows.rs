//! Служба FlexConnect: держит туннель с правами системы и убирает за ним.
//!
//! Запуск без аргументов — режим службы Windows. Команды:
//!   run        работать в консоли (для разработки; TUN потребует прав администратора)
//!   install    скопировать себя и ядра в Program Files и зарегистрировать службу
//!   uninstall  остановить и удалить службу и её папку


use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexconnect_core::ipc::Request;
use crate::supervisor::{Env, Supervisor};
use crate::{pipe, security};
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

const SERVICE_NAME: &str = "FlexConnect";
const DISPLAY_NAME: &str = "FlexConnect VPN";
const DESCRIPTION: &str =
    "Держит VPN-туннель FlexConnect и возвращает сеть в исходное состояние при сбоях.";
const SELF_EXE: &str = "flexconnect-service.exe";
const CORES: [&str; 2] = ["sing-box.exe", "xray.exe"];

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn exe_dir() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent().map(Path::to_path_buf).unwrap_or_default())
}

/// Принимает запросы, пока не поднят флаг `stop`, затем гасит ядра.
fn serve(env: Env, stop: Arc<AtomicBool>) -> std::io::Result<()> {
    let supervisor = Arc::new(Mutex::new(Supervisor::new(env)?));
    let shared = supervisor.clone();
    let result = pipe::serve(stop, move |line| {
        let mut supervisor = shared.lock().unwrap();
        let response = match serde_json::from_str::<Request>(line) {
            Ok(request) => supervisor.handle(request),
            Err(e) => supervisor.bad_request(format!("непонятный запрос: {e}")),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".into())
    });
    supervisor.lock().unwrap().stop();
    result
}

// ---- режим службы Windows ----

define_windows_service!(ffi_service_main, service_main);

fn service_main(_args: Vec<OsString>) {
    // Сообщить об ошибке некому: у службы нет консоли. Код возврата увидит диспетчер служб.
    let _ = run_service();
}

fn status(state: ServiceState, exit_code: u32) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running {
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
        } else {
            ServiceControlAccept::empty()
        },
        exit_code: ServiceExitCode::Win32(exit_code),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

fn run_service() -> AnyResult<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let handler_stop = stop.clone();
    let handle = service_control_handler::register(SERVICE_NAME, move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            handler_stop.store(true, Ordering::SeqCst);
            pipe::wake();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;

    // В режиме службы ядра берутся только из папки установки: она доступна
    // на запись одним администраторам, и подменить ядро обычный пользователь не может.
    let result = (|| -> std::io::Result<()> {
        let bin_dir = exe_dir()?;
        let run_dir = bin_dir.join("run");
        security::create_private_dir(&run_dir)?;
        handle
            .set_service_status(status(ServiceState::Running, 0))
            .map_err(std::io::Error::other)?;
        serve(Env { bin_dir, run_dir }, stop)
    })();

    handle.set_service_status(status(ServiceState::Stopped, if result.is_ok() { 0 } else { 1 }))?;
    Ok(result?)
}

// ---- консольный режим ----

fn run_console() -> AnyResult<()> {
    let bin_dir = match std::env::var_os("FLEXCONNECT_BIN_DIR") {
        Some(dir) => PathBuf::from(dir),
        None if Path::new("bin").join(CORES[0]).is_file() => PathBuf::from("bin"),
        None => exe_dir()?,
    };
    let run_dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("FlexConnect")
        .join("run");
    std::fs::create_dir_all(&run_dir)?;

    let stop = Arc::new(AtomicBool::new(false));
    let handler_stop = stop.clone();
    ctrlc::set_handler(move || {
        handler_stop.store(true, Ordering::SeqCst);
        pipe::wake();
    })?;
    eprintln!("Служба FlexConnect работает в консоли. Ядра: {}. Ctrl+C — выход.", bin_dir.display());
    serve(Env { bin_dir, run_dir }, stop)?;
    eprintln!("Остановлено.");
    Ok(())
}

// ---- установка ----

// Resolve the native system folder through Windows, never through the environment
// inherited from the 32-bit NSIS installer.
fn known_folder(id: &windows_sys::core::GUID) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    let mut value = std::ptr::null_mut();
    let result = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut value) };
    if result < 0 { return Err(std::io::Error::from_raw_os_error(result)); }
    let path = unsafe {
        let mut len = 0;
        while *value.add(len) != 0 { len += 1; }
        let path = PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(value, len)));
        CoTaskMemFree(value.cast());
        path
    };
    Ok(path)
}

fn install_dir() -> AnyResult<PathBuf> {
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFilesX64;
    Ok(known_folder(&FOLDERID_ProgramFilesX64)?.join("FlyxService"))
}

fn copy_unless_same(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() && from.canonicalize()? == to.canonicalize()? { return Ok(()); }
    std::fs::copy(from, to)?;
    Ok(())
}

fn find_core(name: &str) -> AnyResult<PathBuf> {
    let candidates = [exe_dir()?.join(name), Path::new("bin").join(name)];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("не найден {name}: положите его рядом со службой или в папку bin").into())
}

fn install() -> AnyResult<()> {
    // Validate the complete bundle before stopping any existing installation.
    let source = std::env::current_exe()?;
    let cores = CORES.into_iter().map(find_core).collect::<AnyResult<Vec<_>>>()?;
    let dir = install_dir()?;
    let manager = ServiceManager::local_computer(None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
    unregister()?;
    // Only the service and its cores run as SYSTEM. They must live in a
    // protected folder even when the GUI is installed on another drive.
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(SELF_EXE);
    copy_unless_same(&source, &target)?;
    for (source, name) in cores.iter().zip(CORES) {
        copy_unless_same(source, &dir.join(name))?;
    }
    let info = ServiceInfo {
        name: SERVICE_NAME.into(), display_name: DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS, start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal, executable_path: target,
        launch_arguments: vec![], dependencies: vec![], account_name: None, account_password: None,
    };
    let service = manager.create_service(&info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::DELETE)?;
    let started = service.set_description(DESCRIPTION).and_then(|_| service.start::<&str>(&[]));
    if let Err(error) = started {
        let _ = service.delete();
        return Err(error.into());
    }
    println!("Flyx service installed and started.");
    Ok(())
}

fn uninstall() -> AnyResult<()> {
    unregister()?;
    let dir = install_dir()?;
    // Remove only our managed files, never the GUI directory or user settings.
    for file in CORES.iter().copied().chain([SELF_EXE]) {
        match std::fs::remove_file(dir.join(file)) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    match std::fs::remove_dir_all(dir.join("run")) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    // Other files, if present, are deliberately preserved.
    let _ = std::fs::remove_dir(dir);
    Ok(())
}

// Stop and remove the SCM registration before replacing service binaries.
fn unregister() -> AnyResult<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = match manager.open_service(SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE) {
        Ok(service) => service,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let state = service.query_status()?.current_state;
    if state != ServiceState::Stopped {
        if state != ServiceState::StopPending { service.stop()?; }
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while service.query_status()?.current_state != ServiceState::Stopped {
            if std::time::Instant::now() >= deadline { return Err("служба не остановилась за 20 секунд".into()); }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    service.delete()?;
    drop(service);
    // DeleteService is asynchronous while another process holds a handle.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => return Ok(()),
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1072) => (),
            Err(e) => return Err(e.into()),
            Ok(_) => (),
        }
        if std::time::Instant::now() >= deadline { return Err("Service deletion timed out; close Services and retry.".into()); }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn register() -> AnyResult<()> { install() }

fn write_install_error(command: &str, error: &dyn std::fmt::Display) {
    // The helper output is UTF-8, but nsExec decodes console output through the
    // Windows code page. Keep the dialog localized in NSIS and save details here.
    if let Ok(dir) = install_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join("install-error.log"), format!("{command}: {error}\n"));
        }
    }
}

pub fn run() {
    let command = std::env::args().nth(1);
    let result: AnyResult<()> = match command.as_deref() {
        None => service_dispatcher::start(SERVICE_NAME, ffi_service_main).map_err(|e| {
            format!("это программа-служба; для запуска в консоли используйте команду run ({e})").into()
        }),
        Some("run") => run_console(),
        Some("install") => install(),
        Some("uninstall") => uninstall(),
        Some("register") => register(),
        Some("unregister") => unregister(),
        Some(other) => Err(format!("неизвестная команда {other}: есть run, install, uninstall").into()),
    };
    if let Err(e) = result {
        if matches!(command.as_deref(), Some("install" | "uninstall" | "register" | "unregister")) {
            write_install_error(command.as_deref().unwrap(), &e);
        }
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod installer_tests {
    use super::*;

    #[test]
    fn native_service_path_ignores_nsis_environment() {
        // Reproduce the 32-bit installer's environment without changing services.
        let old = std::env::var_os("ProgramFiles");
        let expected = install_dir().unwrap();
        std::env::set_var("ProgramFiles", r"Z:\wrong-program-files-x86");
        let actual = install_dir();
        match old { Some(value) => std::env::set_var("ProgramFiles", value), None => std::env::remove_var("ProgramFiles") }
        assert_eq!(actual.unwrap(), expected);
        assert!(expected.is_absolute());
        assert_eq!(expected.file_name().unwrap(), "FlyxService");
    }

    #[test]
    fn copying_service_to_itself_does_not_truncate_it() {
        let dir = std::env::temp_dir().join(format!("flyx-copy-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("service.exe");
        let dest = dir.join("copy.exe");
        std::fs::write(&source, b"service-payload").unwrap();
        copy_unless_same(&source, &source).unwrap();
        copy_unless_same(&source, &dest).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), b"service-payload");
        assert_eq!(std::fs::read(&dest).unwrap(), b"service-payload");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
