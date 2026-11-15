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

fn install_dir() -> AnyResult<PathBuf> {
    let base = std::env::var_os("ProgramFiles").ok_or("не задана переменная ProgramFiles")?;
    Ok(PathBuf::from(base).join("FlexConnect"))
}

fn find_core(name: &str) -> AnyResult<PathBuf> {
    let candidates = [exe_dir()?.join(name), Path::new("bin").join(name)];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("не найден {name}: положите его рядом со службой или в папку bin").into())
}

fn install() -> AnyResult<()> {
    let dir = install_dir()?;
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| format!("нет доступа к диспетчеру служб (нужны права администратора): {e}"))?;

    // сначала убеждаемся, что всё на месте, и только потом трогаем систему
    let mut cores = Vec::new();
    for core in CORES {
        cores.push((find_core(core)?, dir.join(core)));
    }
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(SELF_EXE);
    std::fs::copy(std::env::current_exe()?, &target)?;
    for (from, to) in cores {
        std::fs::copy(from, to)?;
    }

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: target,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)?;
    service.set_description(DESCRIPTION)?;
    service.start::<&str>(&[])?;
    println!("Служба установлена в {} и запущена.", dir.display());
    Ok(())
}

fn uninstall() -> AnyResult<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("нет доступа к диспетчеру служб (нужны права администратора): {e}"))?;
    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    )?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        service.stop()?;
        for _ in 0..50 {
            if service.query_status()?.current_state == ServiceState::Stopped {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    service.delete()?;
    drop(service);

    let dir = install_dir()?;
    // удаляем только то, что сами положили
    for file in CORES.iter().copied().chain([SELF_EXE]) {
        std::fs::remove_file(dir.join(file)).ok();
    }
    std::fs::remove_dir_all(dir.join("run")).ok();
    std::fs::remove_dir(&dir).ok();
    println!("Служба удалена.");
    Ok(())
}

// Installer entry points deliberately do not delete application files.
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
    Ok(())
}

fn register() -> AnyResult<()> {
    let dir = exe_dir()?.canonicalize()?;
    let program_files = PathBuf::from(std::env::var_os("ProgramFiles").ok_or("ProgramFiles missing")?).canonicalize()?;
    if !dir.starts_with(program_files) { return Err("служба должна быть установлена внутри Program Files".into()); }
    for core in CORES { if !dir.join(core).is_file() { return Err(format!("не найден {core}").into()); } }
    unregister()?;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
    let info = ServiceInfo {
        name: SERVICE_NAME.into(), display_name: DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS, start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal, executable_path: std::env::current_exe()?,
        launch_arguments: vec![], dependencies: vec![], account_name: None, account_password: None,
    };
    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)?;
    service.set_description(DESCRIPTION)?;
    service.start::<&str>(&[])?;
    Ok(())
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
        eprintln!("Ошибка: {e}");
        std::process::exit(1);
    }
}
