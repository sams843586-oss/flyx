#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! Окно FlexConnect: тонкая прослойка между интерфейсом и службой.
//! Туннелем управляет служба; приложение только разбирает ссылки и передаёт запросы.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use flexconnect_core::ipc::{self, ConnectRequest, Request, Response};
use flexconnect_core::link::b64_decode_lenient;
use flexconnect_core::model::Node;
use flexconnect_core::subscription::parse_subscription;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
mod update;

const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;

/// Сервер в том виде, в каком его показывает интерфейс. `node` интерфейс
/// не разбирает, а возвращает как есть при подключении.
#[derive(Serialize)]
struct NodeView {
    name: String,
    server: String,
    port: u16,
    proto: &'static str,
    transport: &'static str,
    security: &'static str,
    via_xray: bool,
    node: Node,
}

#[derive(Serialize)]
struct SkippedView {
    line: usize,
    scheme: Option<String>,
    reason: String,
}

#[derive(Serialize, Default)]
struct Imported {
    /// Адрес подписки, если серверы пришли по ссылке; по нему их можно обновить.
    source: Option<String>,
    title: Option<String>,
    nodes: Vec<NodeView>,
    skipped: Vec<SkippedView>,
    used: Option<u64>,
    total: Option<u64>,
    expire: Option<u64>,
}

fn decode_title(raw: &str) -> String {
    raw.strip_prefix("base64:")
        .and_then(b64_decode_lenient)
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| raw.to_string())
        .trim()
        .to_string()
}

/// Заголовок вида `upload=1; download=2; total=3; expire=4`.
fn apply_userinfo(header: &str, out: &mut Imported) {
    let mut upload = 0u64;
    let mut download = 0u64;
    let mut seen = false;
    for part in header.split(';') {
        let Some((key, value)) = part.split_once('=') else { continue };
        let Ok(value) = value.trim().parse::<u64>() else { continue };
        match key.trim() {
            "upload" => (upload, seen) = (value, true),
            "download" => (download, seen) = (value, true),
            "total" if value > 0 => out.total = Some(value),
            "expire" if value > 0 => out.expire = Some(value),
            _ => {}
        }
    }
    if seen {
        out.used = Some(upload.saturating_add(download));
    }
}

fn fetch_as(url: &str, out: &mut Imported, user_agent: &str) -> Result<String, String> {
    let device_id = flexconnect_core::device::subscription_id()?;
    let tls = native_tls::TlsConnector::new().map_err(|e| format!("не удалось инициализировать TLS: {e}"))?;
    let response = ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(tls))
        .timeout(Duration::from_secs(20))
        .build()
        .get(url)
        .set("User-Agent", user_agent)
        .set("x-hwid", &device_id)
        .set("x-device-os", flexconnect_core::platform::OS_NAME)
        .set("x-device-model", "Flyx Desktop")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("сервер подписки ответил ошибкой {code}"),
            other => format!("не удалось скачать подписку: {other}"),
        })?;
    if let Some(title) = response.header("profile-title") {
        out.title = Some(decode_title(title)).filter(|t| !t.is_empty());
    }
    if let Some(info) = response.header("subscription-userinfo") {
        apply_userinfo(info, out);
    }
    response.into_string().map_err(|e| format!("подписка пришла не текстом: {e}"))
}

fn fetch(url: &str, out: &mut Imported) -> Result<String, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static WORKING_AGENTS: OnceLock<Mutex<HashMap<String,String>>> = OnceLock::new();
    let cache = WORKING_AGENTS.get_or_init(Default::default);
    let mut agents = vec![concat!("Flyx/", env!("CARGO_PKG_VERSION")).to_string(), "v2rayNG/1.10.0".into(), "Happ/3.3.6".into(), "INCY/3.7.2".into()];
    if let Some(previous) = cache.lock().unwrap().get(url).cloned() {
        agents.retain(|agent|agent != &previous); agents.insert(0,previous);
    }
    let mut last_error = None;
    for agent in agents {
        let mut metadata = Imported::default();
        let body = match fetch_as(url,&mut metadata,&agent) {
            Ok(body) => body,
            Err(error) => { last_error = Some(error); continue; },
        };
        if parse_subscription(&body).nodes.is_empty() { continue; }
        out.title = metadata.title.or(out.title.take());
        out.used = metadata.used.or(out.used); out.total = metadata.total.or(out.total); out.expire = metadata.expire.or(out.expire);
        cache.lock().unwrap().insert(url.to_string(),agent);
        return Ok(body);
    }
    Err(last_error.unwrap_or_else(||"провайдер не выдал поддерживаемые серверы: проверены обычный и совместимые форматы. Служебные записи не добавлены".into()))
}

#[tauri::command(async)]
fn import_source(input: String) -> Result<Imported, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("вставьте ссылку на подписку или ключ".into());
    }
    let mut out = Imported::default();
    let body = if input.starts_with("https://") || input.starts_with("http://") {
        out.source = Some(input.to_string());
        fetch(input, &mut out)?
    } else {
        input.to_string()
    };

    let parsed = parse_subscription(&body);
    out.nodes = parsed
        .nodes
        .into_iter()
        .map(|node| NodeView {
            name: node.name.clone(),
            server: node.server.clone(),
            port: node.port,
            proto: node.proto_name(),
            transport: node.transport_name(),
            security: node.security_name(),
            via_xray: flexconnect_core::engine::uses_xray(&node),
            node,
        })
        .collect();
    out.skipped = parsed
        .skipped
        .into_iter()
        .map(|s| SkippedView { line: s.line, scheme: s.scheme, reason: s.reason })
        .collect();
    if out.nodes.is_empty() {
        return Err(match out.skipped.first() {
            Some(s) => format!("серверов не найдено: {}", s.reason),
            None => "серверов не найдено".into(),
        });
    }
    Ok(out)
}

fn ask(request: &Request) -> Result<Response, String> {
    let mut pipe = ipc::open_client().map_err(|_| "служба FlexConnect не запущена".to_string())?;
    let mut line = serde_json::to_string(request).map_err(|e| e.to_string())?;
    line.push('\n');
    pipe.write_all(line.as_bytes()).map_err(|e| format!("служба не приняла запрос: {e}"))?;
    let mut reply = String::new();
    BufReader::new(pipe)
        .read_line(&mut reply)
        .map_err(|e| format!("служба не ответила: {e}"))?;
    serde_json::from_str(&reply).map_err(|_| "служба прислала непонятный ответ".to_string())
}

#[tauri::command(async)]
fn svc_status() -> Result<Response, String> {
    ask(&Request::Status)
}

#[tauri::command(async)]
fn svc_connect(mut request: ConnectRequest) -> Result<Response, String> {
    request.node = flexconnect_core::engine::service_node(request.node);
    ask(&Request::Connect(request))
}

#[tauri::command(async)]
fn svc_disconnect() -> Result<Response, String> {
    ask(&Request::Disconnect)
}

#[tauri::command(async)]
fn svc_logs(lines: usize) -> Result<Response, String> {
    ask(&Request::Logs { lines })
}

/// Время установки TCP-соединения с сервером, мс. `None` — сервер не ответил
/// (для серверов на UDP это нормально).
#[tauri::command(async)]
fn ping(server: String, port: u16) -> Option<u32> {
    let addr = (server.as_str(), port).to_socket_addrs().ok()?.next()?;
    let started = Instant::now();
    TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok()?;
    Some(started.elapsed().as_millis() as u32)
}

fn state_path() -> Result<PathBuf, String> {
    Ok(flexconnect_core::platform::data_dir()?.join("state.json"))
}

#[tauri::command(async)]
fn load_state() -> Value {
    state_path()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

#[tauri::command(async)]
fn save_state(state: Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("слишком много данных для сохранения".into());
    }
    let path = state_path()?;
    if let Some(dir) = path.parent() {
        flexconnect_core::platform::private_dir(dir).map_err(|e| e.to_string())?;
    }
    // пишем во временный файл и переименовываем: сбой не оставит полфайла
    let tmp = path.with_extension("json.tmp");
    flexconnect_core::platform::write_private(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_ad_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_control() || c == '"' || c == '\\') {
        return Err("рекламная ссылка должна использовать HTTPS".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", &url])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| format!("не удалось открыть браузер: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(&url).spawn()
        .map_err(|e| format!("не удалось открыть браузер: {e}"))?;
    Ok(())
}

#[tauri::command]
fn platform_info() -> &'static str { flexconnect_core::platform::OS_NAME }

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("app-background", false);
    }
}

struct TrayItems {
    open: MenuItem<tauri::Wry>,
    hide: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn tray_labels(language: &str) -> [&'static str; 3] {
    match language {
        "en" => ["Open Flyx", "Hide to tray", "Disconnect and exit"],
        "zh" => ["打开 Flyx", "隐藏到托盘", "断开并退出"],
        "fa" => ["بازکردن Flyx", "پنهان‌کردن در سینی سیستم", "قطع اتصال و خروج"],
        "ko" => ["Flyx 열기", "트레이로 숨기기", "연결 해제 후 종료"],
        "tk" => ["Flyx aç", "Treýe gizle", "Aýyr we çyk"],
        "uz" => ["Flyxni ochish", "Treyga yashirish", "Uzish va chiqish"],
        "az" => ["Flyx-i aç", "Treyə gizlət", "Ayır və çıx"],
        "kk" => ["Flyx ашу", "Трейге жасыру", "Ажырату және шығу"],
        _ => ["Открыть Flyx", "Скрыть в трей", "Отключить VPN и выйти"],
    }
}

#[tauri::command]
fn set_ui_language(language: String, items: tauri::State<'_, TrayItems>) -> Result<(), String> {
    let labels = tray_labels(&language);
    items.open.set_text(labels[0]).map_err(|e|e.to_string())?;
    items.hide.set_text(labels[1]).map_err(|e|e.to_string())?;
    items.quit.set_text(labels[2]).map_err(|e|e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::PendingUpdate::new(None))
        .setup(|app| {
            let saved = load_state();
            let language = saved["preferences"]["language"].as_str().unwrap_or("ru");
            let labels = tray_labels(language);
            let open = MenuItem::with_id(app, "open", labels[0], true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", labels[1], true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", labels[2], true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &hide, &quit])?;
            let icon = app.default_window_icon().cloned().ok_or("нет значка приложения")?;
            TrayIconBuilder::with_id("flyx-tray")
                .icon(icon)
                .tooltip("Flyx VPN")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if matches!(event, TrayIconEvent::Click {
                        button: MouseButton::Left, button_state: MouseButtonState::Up, ..
                    }) {
                        show_main_window(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main_window(app),
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                            let _ = window.emit("app-background", true);
                        }
                    }
                    "quit" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let response = tauri::async_runtime::spawn_blocking(|| ask(&Request::Disconnect)).await;
                            if let Ok(Ok(response)) = response {
                                if !response.ok {
                                    show_main_window(&app);
                                    let _ = app.emit("tray-error", response.error.unwrap_or_else(|| "не удалось отключить VPN".into()));
                                    return;
                                }
                            }
                            app.exit(0);
                        });
                    }
                    _ => {}
                })
                .build(app)?;
            app.manage(TrayItems { open, hide, quit });
            let start_hidden = saved["preferences"]["startHidden"].as_bool().unwrap_or(false);
            if !start_hidden && !std::env::args().any(|arg| arg == "--background") {
                show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.emit("app-background", true);
                } else if matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(_)) {
                    let background = !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false);
                    let _ = window.emit("app-background", background);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            import_source,
            open_ad_url,
            svc_status,
            svc_connect,
            svc_disconnect,
            svc_logs,
            ping,
            load_state,
            save_state,
            set_ui_language,
            platform_info,
            update::check_update, update::install_update
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_retries_after_provider_notice() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for expected in ["Flyx/", "v2rayNG/"] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.contains(expected));
                assert!(request.to_ascii_lowercase().contains("x-hwid:"));
                let body = if expected == "Flyx/" { "vless://uuid@127.0.0.1:443#Данное%20приложение%20не%20поддерживается!" } else { "trojan://pw@example.com:443#Test" };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\nProfile-Title: Test provider\r\nSubscription-Userinfo: download=20; total=100\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let imported = import_source(format!("http://{address}/subscription")).unwrap();
        server.join().unwrap();
        assert_eq!(imported.nodes.len(), 1);
        assert_eq!(imported.title.as_deref(), Some("Test provider"));
        assert_eq!(imported.used, Some(20));
        assert_eq!(imported.total, Some(100));
        assert!(imported.source.is_some());
    }

    #[test]
    fn userinfo_header() {
        let mut out = Imported::default();
        apply_userinfo("upload=100; download=250; total=1000; expire=1790000000", &mut out);
        assert_eq!(out.used, Some(350));
        assert_eq!(out.total, Some(1000));
        assert_eq!(out.expire, Some(1790000000));
    }

    #[test]
    fn unlimited_userinfo() {
        let mut out = Imported::default();
        apply_userinfo("upload=0; download=5; total=0; expire=0", &mut out);
        assert_eq!(out.used, Some(5));
        assert_eq!(out.total, None);
        assert_eq!(out.expire, None);
    }

    #[test]
    fn titles() {
        assert_eq!(decode_title("base64:0J/RgNC40LLQtdGC"), "Привет");
        assert_eq!(decode_title("Plain VPN"), "Plain VPN");
    }

    #[test]
    fn import_plain_keys() {
        let out = import_source("trojan://pw@example.com:443#One\nwireguard://x@y:1".into()).unwrap();
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].proto, "trojan");
        assert_eq!(out.skipped.len(), 1);
        assert!(out.source.is_none());
        assert!(import_source("  ".into()).is_err());
        assert!(import_source("мусор".into()).is_err());
    }
}
