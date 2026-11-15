//! Протокол между интерфейсом и службой: одна строка JSON — запрос, одна — ответ.
//!
//! Служба работает с правами системы, а запросы приходят от обычного пользователя,
//! поэтому в запросе нет ни путей к файлам, ни готовых конфигов: только сервер
//! и настройки, из которых служба сама собирает конфиги.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::model::Node;
use crate::singbox::{Mode, Options, Split};

pub const PIPE_NAME: &str = r"\\.\pipe\FlexConnect";
#[cfg(target_os = "linux")]
pub const SOCKET_PATH: &str = "/run/flexconnect/control.sock";
/// Предел размера запроса, байт.
pub const MAX_REQUEST: usize = 64 * 1024;

const MAX_APPS: usize = 256;
const MAX_DOMAINS: usize = 4096;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Connect(ConnectRequest),
    Disconnect,
    Logs {
        #[serde(default)]
        lines: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelMode {
    #[default]
    Tun,
    Proxy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    #[default]
    All,
    /// Приложения из `apps` идут мимo VPN.
    Except,
    /// Через VPN идут только приложения из `apps`.
    Only,
}

fn default_proxy_port() -> u16 {
    2080
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub node: Node,
    #[serde(default)]
    pub mode: TunnelMode,
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default)]
    pub split: SplitMode,
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub direct_domains: Vec<String>,
    #[serde(default)]
    pub direct_dns: Option<String>,
    #[serde(default)]
    pub remote_dns: Option<String>,
}

fn check_dns(value: &Option<String>, default: &str) -> Result<String, String> {
    match value {
        None => Ok(default.to_string()),
        Some(v) => v
            .parse::<IpAddr>()
            .map(|ip| ip.to_string())
            .map_err(|_| format!("адрес DNS должен быть IP-адресом, а не «{v}»")),
    }
}

impl ConnectRequest {
    /// Проверяет запрос и превращает его в настройки генератора конфигов.
    pub fn options(&self) -> Result<Options, String> {
        if self.apps.len() > MAX_APPS {
            return Err(format!("слишком много приложений: больше {MAX_APPS}"));
        }
        if self.direct_domains.len() > MAX_DOMAINS {
            return Err(format!("слишком много доменов: больше {MAX_DOMAINS}"));
        }
        if self.mode == TunnelMode::Proxy && self.proxy_port < 1024 {
            return Err("порт локального прокси должен быть не меньше 1024".into());
        }
        let defaults = Options::default();
        Ok(Options {
            mode: match self.mode {
                TunnelMode::Tun => Mode::Tun,
                TunnelMode::Proxy => Mode::Proxy { port: self.proxy_port },
            },
            split: match self.split {
                SplitMode::All => Split::All,
                SplitMode::Except => Split::Except(self.apps.clone()),
                SplitMode::Only => Split::Only(self.apps.clone()),
            },
            direct_suffixes: self.direct_domains.clone(),
            direct_dns: check_dns(&self.direct_dns, &defaults.direct_dns)?,
            remote_dns: check_dns(&self.remote_dns, &defaults.remote_dns)?,
            log_level: defaults.log_level,
            // запрос разбирается в момент подключения, на том компьютере, где поднимется туннель
            ipv6: crate::platform::host_has_ipv6(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Connected,
    /// Подключение не удалось или оборвалось; причина в `error`.
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// Выполнен ли сам запрос.
    pub ok: bool,
    pub state: State,
    #[serde(default)]
    pub server: Option<String>,
    /// Время подключения, секунды Unix.
    #[serde(default)]
    pub since: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
    pub version: String,
}

/// Открывает канал к службе со стороны клиента.
///
/// Права запрашиваются поштучно: обычный «доступ на запись» включает право
/// создавать экземпляры канала, а его служба клиентам не даёт.
#[cfg(windows)]
pub fn open_client() -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    const FILE_WRITE_DATA: u32 = 0x0002;
    std::fs::OpenOptions::new()
        .access_mode(FILE_GENERIC_READ | FILE_WRITE_DATA)
        .open(PIPE_NAME)
}

#[cfg(target_os = "linux")]
pub fn open_client() -> std::io::Result<std::os::unix::net::UnixStream> {
    use std::time::Duration;
    let stream = std::os::unix::net::UnixStream::connect(SOCKET_PATH)?;
    stream.set_read_timeout(Some(Duration::from_secs(35)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::parse_link;

    fn request(extra: &str) -> ConnectRequest {
        let node = serde_json::to_string(&parse_link("trojan://pw@example.com:443").unwrap()).unwrap();
        serde_json::from_str(&format!(r#"{{"node":{node}{extra}}}"#)).unwrap()
    }

    #[test]
    fn request_round_trip() {
        let req = Request::Connect(request(r#","mode":"proxy","split":"only","apps":["a.exe"]"#));
        let text = serde_json::to_string(&req).unwrap();
        assert!(text.starts_with(r#"{"cmd":"connect""#));
        let Request::Connect(back) = serde_json::from_str(&text).unwrap() else {
            panic!("не тот запрос");
        };
        assert_eq!(back.node.server, "example.com");
        let opts = back.options().unwrap();
        assert_eq!(opts.mode, Mode::Proxy { port: 2080 });
        assert_eq!(opts.split, Split::Only(vec!["a.exe".into()]));
    }

    #[test]
    fn defaults() {
        let opts = request("").options().unwrap();
        assert_eq!(opts.mode, Mode::Tun);
        assert_eq!(opts.split, Split::All);
        assert_eq!(opts.direct_dns, "77.88.8.8");
    }

    #[test]
    fn dns_must_be_an_ip() {
        assert!(request(r#","direct_dns":"8.8.8.8""#).options().is_ok());
        assert!(request(r#","direct_dns":"evil.example/x""#).options().is_err());
    }

    #[test]
    fn privileged_proxy_port_is_rejected() {
        assert!(request(r#","mode":"proxy","proxy_port":80"#).options().is_err());
    }

    #[test]
    fn simple_requests() {
        assert!(matches!(serde_json::from_str(r#"{"cmd":"status"}"#).unwrap(), Request::Status));
        assert!(matches!(
            serde_json::from_str(r#"{"cmd":"logs","lines":5}"#).unwrap(),
            Request::Logs { lines: 5 }
        ));
        assert!(serde_json::from_str::<Request>(r#"{"cmd":"format_c"}"#).is_err());
    }
}
