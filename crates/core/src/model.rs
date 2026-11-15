use serde::{Deserialize, Serialize};

/// Один сервер из ссылки или подписки, независимо от ядра.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub proto: Proto,
    pub transport: Transport,
    pub tls: Option<Tls>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Proto {
    Vless {
        uuid: String,
        flow: Option<String>,
        /// Шифрование VLESS поверх транспорта; `None` — обычное `none`.
        encryption: Option<String>,
    },
    Vmess {
        uuid: String,
        alter_id: u32,
        security: String,
    },
    Trojan {
        password: String,
    },
    Shadowsocks {
        method: String,
        password: String,
    },
    Hysteria2 {
        password: String,
        obfs_password: Option<String>,
    },
    Tuic {
        uuid: String,
        password: String,
        congestion_control: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transport {
    Tcp,
    Ws { path: String, host: Option<String> },
    Grpc { service_name: String },
    HttpUpgrade { path: String, host: Option<String> },
    Http { path: String, host: Option<String> },
    /// Есть только в xray-core; sing-box его не умеет.
    Xhttp {
        path: String,
        host: Option<String>,
        mode: Option<String>,
        /// Дополнительные настройки XHTTP в виде JSON, как их передают в ссылке.
        extra: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Tls {
    pub sni: Option<String>,
    pub alpn: Vec<String>,
    pub insecure: bool,
    pub fingerprint: Option<String>,
    pub reality: Option<Reality>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reality {
    pub public_key: String,
    pub short_id: String,
}

impl Node {
    pub fn proto_name(&self) -> &'static str {
        match self.proto {
            Proto::Vless { .. } => "vless",
            Proto::Vmess { .. } => "vmess",
            Proto::Trojan { .. } => "trojan",
            Proto::Shadowsocks { .. } => "shadowsocks",
            Proto::Hysteria2 { .. } => "hysteria2",
            Proto::Tuic { .. } => "tuic",
        }
    }

    pub fn transport_name(&self) -> &'static str {
        match self.transport {
            Transport::Tcp => "tcp",
            Transport::Ws { .. } => "ws",
            Transport::Grpc { .. } => "grpc",
            Transport::HttpUpgrade { .. } => "httpupgrade",
            Transport::Http { .. } => "http",
            Transport::Xhttp { .. } => "xhttp",
        }
    }

    pub fn security_name(&self) -> &'static str {
        match &self.tls {
            None => "none",
            Some(t) if t.reality.is_some() => "reality",
            Some(_) => "tls",
        }
    }

    /// Сервер, который sing-box сам не поднимет: нужен xray-core как второе ядро.
    pub fn needs_xray(&self) -> bool {
        self.xray_reason().is_some()
    }

    /// Что именно в сервере не умеет sing-box.
    pub fn xray_reason(&self) -> Option<&'static str> {
        if matches!(self.transport, Transport::Xhttp { .. }) {
            return Some("XHTTP");
        }
        if matches!(&self.proto, Proto::Vless { encryption: Some(_), .. }) {
            return Some("шифрование VLESS");
        }
        None
    }
}
