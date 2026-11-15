//! Конфиг xray-core для серверов, которые sing-box не поддерживает.
//! xray здесь только «последняя миля»: локальный SOCKS-вход и один выход на сервер.

use serde_json::{json, Map, Value};

use crate::model::*;
use crate::singbox::ConfigError;

/// Имя процесса, по которому sing-box исключает трафик xray из туннеля.
pub const PROCESS_NAME: &str = crate::platform::XRAY;

fn invalid<T>(msg: impl Into<String>) -> Result<T, ConfigError> {
    Err(ConfigError::Invalid(msg.into()))
}

fn put(m: &mut Map<String, Value>, key: &str, value: &Option<String>) {
    if let Some(v) = value {
        m.insert(key.into(), json!(v));
    }
}

fn stream_settings(node: &Node) -> Result<Value, ConfigError> {
    let mut s = Map::new();

    match &node.transport {
        Transport::Tcp => {
            s.insert("network".into(), json!("tcp"));
        }
        Transport::Ws { path, host } => {
            let mut w = Map::new();
            w.insert("path".into(), json!(path));
            put(&mut w, "host", host);
            s.insert("network".into(), json!("ws"));
            s.insert("wsSettings".into(), Value::Object(w));
        }
        Transport::Grpc { service_name } => {
            s.insert("network".into(), json!("grpc"));
            s.insert("grpcSettings".into(), json!({ "serviceName": service_name }));
        }
        Transport::HttpUpgrade { path, host } => {
            let mut h = Map::new();
            h.insert("path".into(), json!(path));
            put(&mut h, "host", host);
            s.insert("network".into(), json!("httpupgrade"));
            s.insert("httpupgradeSettings".into(), Value::Object(h));
        }
        Transport::Http { .. } => {
            return invalid("транспорт HTTP/2 в xray-core убран; такой сервер идёт через sing-box");
        }
        Transport::Xhttp { path, host, mode, extra } => {
            let mut x = Map::new();
            x.insert("path".into(), json!(path));
            put(&mut x, "host", host);
            put(&mut x, "mode", mode);
            if let Some(extra) = extra {
                let parsed: Value = serde_json::from_str(extra).map_err(|_| {
                    ConfigError::Invalid("параметр extra в ссылке — не JSON".into())
                })?;
                if !parsed.is_object() {
                    return invalid("параметр extra в ссылке должен быть JSON-объектом");
                }
                x.insert("extra".into(), parsed);
            }
            s.insert("network".into(), json!("xhttp"));
            s.insert("xhttpSettings".into(), Value::Object(x));
        }
    }

    match &node.tls {
        None => {
            s.insert("security".into(), json!("none"));
        }
        Some(tls) => match &tls.reality {
            Some(r) => {
                let mut m = Map::new();
                put(&mut m, "serverName", &tls.sni);
                m.insert(
                    "fingerprint".into(),
                    json!(crate::fingerprint::normalize(tls.fingerprint.as_deref().unwrap_or("chrome"))),
                );
                m.insert("publicKey".into(), json!(r.public_key));
                m.insert("shortId".into(), json!(r.short_id));
                s.insert("security".into(), json!("reality"));
                s.insert("realitySettings".into(), Value::Object(m));
            }
            None => {
                let mut m = Map::new();
                put(&mut m, "serverName", &tls.sni);
                if let Some(fp) = &tls.fingerprint {
                    m.insert("fingerprint".into(), json!(crate::fingerprint::normalize(fp)));
                }
                if !tls.alpn.is_empty() {
                    m.insert("alpn".into(), json!(tls.alpn));
                }
                if tls.insecure {
                    m.insert("allowInsecure".into(), json!(true));
                }
                s.insert("security".into(), json!("tls"));
                s.insert("tlsSettings".into(), Value::Object(m));
            }
        },
    }
    Ok(Value::Object(s))
}

pub fn outbound(node: &Node) -> Result<Value, ConfigError> {
    let (protocol, settings) = match &node.proto {
        Proto::Vless { uuid, flow, encryption } => {
            let mut user = Map::new();
            user.insert("id".into(), json!(uuid));
            user.insert(
                "encryption".into(),
                json!(encryption.as_deref().unwrap_or("none")),
            );
            put(&mut user, "flow", flow);
            (
                "vless",
                json!({ "vnext": [{ "address": node.server, "port": node.port, "users": [user] }] }),
            )
        }
        Proto::Vmess { uuid, alter_id, security } => (
            "vmess",
            json!({ "vnext": [{
                "address": node.server,
                "port": node.port,
                "users": [{ "id": uuid, "alterId": alter_id, "security": security }]
            }] }),
        ),
        Proto::Trojan { password } => (
            "trojan",
            json!({ "servers": [{ "address": node.server, "port": node.port, "password": password }] }),
        ),
        Proto::Shadowsocks { method, password } => (
            "shadowsocks",
            json!({ "servers": [{
                "address": node.server, "port": node.port, "method": method, "password": password
            }] }),
        ),
        Proto::Hysteria2 { .. } | Proto::Tuic { .. } => {
            return invalid(format!("{} идёт через sing-box, а не xray", node.proto_name()));
        }
    };
    Ok(json!({
        "tag": "proxy",
        "protocol": protocol,
        "settings": settings,
        "streamSettings": stream_settings(node)?
    }))
}

/// Конфиг xray: SOCKS на `127.0.0.1:socks_port` → сервер.
pub fn build_config(node: &Node, socks_port: u16, log_level: &str) -> Result<Value, ConfigError> {
    // у xray свои названия уровней
    let level = match log_level {
        "trace" | "debug" => "debug",
        "info" => "info",
        "error" | "fatal" | "panic" => "error",
        _ => "warning",
    };
    Ok(json!({
        "log": { "loglevel": level },
        "inbounds": [{
            "tag": "from-singbox",
            "listen": "127.0.0.1",
            "port": socks_port,
            "protocol": "socks",
            "settings": { "auth": "noauth", "udp": true }
        }],
        "outbounds": [outbound(node)?]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::parse_link;

    #[test]
    fn vless_xhttp_reality() {
        let n = parse_link(
            "vless://uuid@example.com:443?type=xhttp&security=reality&pbk=KEY&sid=ab&sni=example.org&path=%2Fx&mode=packet-up&extra=%7B%22xPaddingBytes%22%3A%22100-1000%22%7D",
        )
        .unwrap();
        let c = build_config(&n, 20808, "info").unwrap();
        assert_eq!(c["inbounds"][0]["port"], 20808);
        let o = &c["outbounds"][0];
        assert_eq!(o["protocol"], "vless");
        assert_eq!(o["settings"]["vnext"][0]["users"][0]["encryption"], "none");
        let s = &o["streamSettings"];
        assert_eq!(s["network"], "xhttp");
        assert_eq!(s["xhttpSettings"]["mode"], "packet-up");
        assert_eq!(s["xhttpSettings"]["extra"]["xPaddingBytes"], "100-1000");
        assert_eq!(s["security"], "reality");
        assert_eq!(s["realitySettings"]["publicKey"], "KEY");
        assert_eq!(s["realitySettings"]["fingerprint"], "chrome");
    }

    #[test]
    fn broken_extra_is_rejected() {
        let n = parse_link("vless://uuid@example.com:443?type=xhttp&security=tls&extra=oops").unwrap();
        assert!(build_config(&n, 1, "info").is_err());
    }

    #[test]
    fn trojan_ws_tls() {
        let n = parse_link("trojan://pw@example.com:443?type=ws&path=%2Fw&host=h.example.com&sni=h.example.com").unwrap();
        let o = outbound(&n).unwrap();
        assert_eq!(o["settings"]["servers"][0]["password"], "pw");
        assert_eq!(o["streamSettings"]["wsSettings"]["host"], "h.example.com");
        assert_eq!(o["streamSettings"]["tlsSettings"]["serverName"], "h.example.com");
    }

    #[test]
    fn quic_protocols_are_not_for_xray() {
        let n = parse_link("hy2://pw@example.com:443").unwrap();
        assert!(outbound(&n).is_err());
    }
}
