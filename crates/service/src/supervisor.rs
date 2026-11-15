//! Надзор за ядрами: запуск, слежение, остановка и уборка.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flexconnect_core::engine::{self, Plan};
use flexconnect_core::ipc::{ConnectRequest, Request, Response, State};

use crate::job::Job;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_LOG_LINES: usize = 500;
/// Сколько ждём от sing-box сообщения о запуске.
const START_TIMEOUT: Duration = Duration::from_secs(15);
const STARTED_MARK: &str = "sing-box started";

type Logs = Arc<Mutex<VecDeque<String>>>;

/// Где лежат ядра и куда писать конфиги. Задаётся службой, а не клиентом.
pub struct Env {
    pub bin_dir: PathBuf,
    pub run_dir: PathBuf,
}

struct ConfigFiles {
    singbox: PathBuf,
    xray: Option<PathBuf>,
}

impl ConfigFiles {
    fn write(dir: &Path, plan: &Plan) -> Result<Self, String> {
        let files = ConfigFiles {
            singbox: dir.join("sing-box.json"),
            xray: plan.xray.as_ref().map(|_| dir.join("xray.json")),
        };
        let save = |path: &Path, value| -> Result<(), String> {
            let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
            flexconnect_core::platform::write_private(path, &bytes).map_err(|e| format!("не удалось записать конфиг: {e}"))
        };
        save(&files.singbox, &plan.singbox)?;
        if let (Some(path), Some(config)) = (&files.xray, &plan.xray) {
            save(path, config)?;
        }
        Ok(files)
    }
}

impl Drop for ConfigFiles {
    fn drop(&mut self) {
        std::fs::remove_file(&self.singbox).ok();
        if let Some(path) = &self.xray {
            std::fs::remove_file(path).ok();
        }
    }
}

/// Процесс ядра; завершается при уничтожении.
struct Core(Child);

impl Core {
    /// Код завершения, если ядро уже не работает.
    fn exited(&mut self) -> Option<String> {
        match self.0.try_wait() {
            Ok(Some(status)) => Some(status.to_string()),
            Ok(None) => None,
            Err(e) => Some(e.to_string()),
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

struct Session {
    server: String,
    since: u64,
    // порядок полей — порядок остановки: сначала туннель, потом xray, потом конфиги
    singbox: Core,
    xray: Option<Core>,
    _files: ConfigFiles,
}

pub struct Supervisor {
    env: Env,
    job: Job,
    session: Option<Session>,
    last_error: Option<String>,
    logs: Logs,
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // пропускаем до конца управляющей последовательности вида ESC[…m
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn pump(source: impl Read + Send + 'static, prefix: &'static str, logs: Logs) {
    std::thread::spawn(move || {
        for line in BufReader::new(source).lines() {
            let Ok(line) = line else { break };
            let mut logs = logs.lock().unwrap();
            if logs.len() == MAX_LOG_LINES {
                logs.pop_front();
            }
            logs.push_back(format!("[{prefix}] {}", strip_ansi(&line)));
        }
    });
}

fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("не нашлось свободного локального порта: {e}"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Supervisor {
    pub fn new(env: Env) -> std::io::Result<Self> {
        Ok(Supervisor {
            env,
            job: Job::new()?,
            session: None,
            last_error: None,
            logs: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    pub fn handle(&mut self, request: Request) -> Response {
        match request {
            Request::Status => {
                self.refresh();
                self.response(true, None)
            }
            Request::Connect(req) => match self.connect(&req) {
                Ok(()) => self.response(true, None),
                Err(e) => {
                    // если прежнее подключение уцелело, состояние остаётся «подключено»
                    if self.session.is_none() {
                        self.last_error = Some(e.clone());
                    }
                    self.response(false, Some(e))
                }
            },
            Request::Disconnect => {
                self.stop();
                self.last_error = None;
                self.response(true, None)
            }
            Request::Logs { lines } => {
                self.refresh();
                let mut response = self.response(true, None);
                let logs = self.logs.lock().unwrap();
                let count = if lines == 0 { 50 } else { lines }.min(logs.len());
                response.logs = logs.iter().skip(logs.len() - count).cloned().collect();
                response
            }
        }
    }

    /// Ответ на запрос, который не удалось разобрать.
    pub fn bad_request(&mut self, error: String) -> Response {
        self.refresh();
        self.response(false, Some(error))
    }

    /// Останавливает ядра и убирает конфиги.
    pub fn stop(&mut self) {
        self.session = None;
    }

    fn response(&self, ok: bool, error: Option<String>) -> Response {
        let state = match (&self.session, &self.last_error) {
            (Some(_), _) => State::Connected,
            (None, Some(_)) => State::Failed,
            (None, None) => State::Idle,
        };
        Response {
            ok,
            state,
            server: self.session.as_ref().map(|s| s.server.clone()),
            since: self.session.as_ref().map(|s| s.since),
            error: error.or_else(|| self.last_error.clone()),
            logs: Vec::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Замечает, что ядро умерло само, и убирает за ним.
    fn refresh(&mut self) {
        let Some(session) = &mut self.session else { return };
        let died = if let Some(status) = session.singbox.exited() {
            Some(format!("ядро sing-box неожиданно завершилось ({status})"))
        } else {
            session
                .xray
                .as_mut()
                .and_then(|x| x.exited())
                .map(|status| format!("ядро xray неожиданно завершилось ({status})"))
        };
        if let Some(reason) = died {
            self.stop();
            self.last_error = Some(self.with_last_log(reason));
        }
    }

    fn with_last_log(&self, reason: String) -> String {
        let logs = self.logs.lock().unwrap();
        let hint = logs
            .iter()
            .rev()
            .find(|l| l.contains("FATAL") || l.contains("ERROR") || l.contains("Failed"))
            .or_else(|| logs.back());
        match hint {
            Some(line) => format!("{reason}: {line}"),
            None => reason,
        }
    }

    fn spawn(&self, file: &str, prefix: &'static str, args: &[&Path]) -> Result<Core, String> {
        let bin = self.env.bin_dir.join(file);
        #[cfg(target_os = "linux")]
        crate::linux::check_root_owned(&bin, false).map_err(|e| e.to_string())?;
        let mut command = Command::new(&bin);
        command.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        #[cfg(target_os = "linux")]
        crate::job::prepare(&mut command);
        let mut child = command.spawn()
            .map_err(|e| format!("не удалось запустить {}: {e}", bin.display()))?;
        if let Some(out) = child.stdout.take() {
            pump(out, prefix, self.logs.clone());
        }
        if let Some(err) = child.stderr.take() {
            pump(err, prefix, self.logs.clone());
        }
        let core = Core(child);
        self.job
            .assign(&core.0)
            .map_err(|e| format!("не удалось взять {file} под надзор: {e}"))?;
        Ok(core)
    }

    fn connect(&mut self, req: &ConnectRequest) -> Result<(), String> {
        // Сначала проверяем запрос: негодный не должен рвать работающее подключение.
        let opts = req.options()?;
        let plan = engine::plan(&req.node, &opts, free_port()?).map_err(|e| e.to_string())?;

        self.stop();
        self.last_error = None;
        self.logs.lock().unwrap().clear();
        let files = ConfigFiles::write(&self.env.run_dir, &plan)?;

        let xray = match &files.xray {
            Some(config) => Some(self.spawn(
                flexconnect_core::platform::XRAY,
                "xray",
                &[Path::new("run"), Path::new("-c"), config],
            )?),
            None => None,
        };
        let singbox = self.spawn(
            flexconnect_core::platform::SINGBOX,
            "sing-box",
            &[
                Path::new("run"),
                Path::new("-c"),
                &files.singbox,
                Path::new("-D"),
                &self.env.run_dir,
            ],
        )?;
        let mut session = Session {
            server: req.node.name.clone(),
            since: now(),
            singbox,
            xray,
            _files: files,
        };

        let deadline = std::time::Instant::now() + START_TIMEOUT;
        loop {
            if let Some(status) = session.singbox.exited() {
                drop(session);
                return Err(self.with_last_log(format!("sing-box не запустился ({status})")));
            }
            if let Some(status) = session.xray.as_mut().and_then(|x| x.exited()) {
                drop(session);
                return Err(self.with_last_log(format!("xray не запустился ({status})")));
            }
            if self.logs.lock().unwrap().iter().any(|l| l.contains(STARTED_MARK)) {
                break;
            }
            if std::time::Instant::now() > deadline {
                drop(session);
                return Err("ядро не сообщило о запуске за 15 секунд".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.session = Some(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn ansi_colors_are_removed() {
        assert_eq!(strip_ansi("\u{1b}[36mINFO\u{1b}[0m started"), "INFO started");
        assert_eq!(strip_ansi("plain"), "plain");
    }
}
