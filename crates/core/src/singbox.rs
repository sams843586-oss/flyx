//! Генерация конфига sing-box (формат 1.12+) из сервера и настроек маршрутизации.

use std::fmt;

use serde_json::{json, Map, Value};

use crate::model::*;

const PROXY: &str = "proxy";
const DIRECT: &str = "direct";
const DNS_REMOTE: &str = "dns-remote";
const DNS_DIRECT: &str = "dns-direct";

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Виртуальный адаптер: ловит весь трафик системы. Нужны права администратора.
    Tun,
    /// Локальный HTTP/SOCKS-прокси на 127.0.0.1.
    Proxy { port: u16 },
}

/// Раздельное туннелирование по приложениям (имена exe-файлов).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Split {
    /// Всё через VPN.
    #[default]
    All,
    /// Всё через VPN, кроме перечисленных приложений.
    Except(Vec<String>),
    /// Через VPN только перечисленные приложения.
    Only(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub split: Split,
    /// Домены (с поддоменами), которые идут напрямую: `ru`, `yandex.net`.
    pub direct_suffixes: Vec<String>,
    /// DNS для прямого трафика и для адреса самого сервера.
    pub direct_dns: String,
    /// DNS для трафика через VPN; запросы уходят в туннель.
    pub remote_dns: String,
    pub log_level: String,
    /// Объявлять ли туннелю IPv6. Выключается, если у компьютера нет IPv6-интернета
    /// (см. `platform::host_has_ipv6`), иначе прямой трафик пойдёт в никуда.
    pub ipv6: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Tun,
            split: Split::All,
            direct_suffixes: Vec::new(),
            direct_dns: "77.88.8.8".into(),
            remote_dns: "1.1.1.1".into(),
            log_level: "info".into(),
            ipv6: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// Сервер требует xray-core (например, транспорт XHTTP).
    NeedsXray(&'static str),
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NeedsXray(what) => {
                write!(f, "{what} умеет только xray-core, а не sing-box")
            }
            ConfigError::Invalid(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ConfigError {}

fn tls_json(tls: &Tls) -> Value {
    let mut m = Map::new();
    m.insert("enabled".into(), json!(true));
    if let Some(sni) = &tls.sni {
        m.insert("server_name".into(), json!(sni));
    }
    if tls.insecure {
        m.insert("insecure".into(), json!(true));
    }
    if !tls.alpn.is_empty() {
        m.insert("alpn".into(), json!(tls.alpn));
    }
    // Reality без uTLS не работает, поэтому отпечаток по умолчанию — chrome.
    let fingerprint = tls
        .fingerprint
        .as_deref()
        .or(tls.reality.as_ref().map(|_| "chrome"));
    if let Some(fp) = fingerprint {
        m.insert("utls".into(), json!({ "enabled": true, "fingerprint": crate::fingerprint::normalize(fp) }));
    }
    if let Some(r) = &tls.reality {
        m.insert(
            "reality".into(),
            json!({ "enabled": true, "public_key": r.public_key, "short_id": r.short_id }),
        );
    }
    Value::Object(m)
}

/// QUIC-протоколам uTLS не нужен и не поддерживается.
fn quic_tls_json(tls: &Tls) -> Value {
    let mut plain = tls.clone();
    plain.fingerprint = None;
    plain.reality = None;
    tls_json(&plain)
}

/// `/path?ed=2048` — так в ссылках передают early data для WebSocket.
fn split_early_data(path: &str) -> (String, Option<u32>) {
    let Some((base, query)) = path.split_once('?') else {
        return (path.to_string(), None);
    };
    let mut ed = None;
    let rest: Vec<&str> = query
        .split('&')
        .filter(|kv| match kv.strip_prefix("ed=") {
            Some(v) => {
                ed = v.parse().ok();
                false
            }
            None => true,
        })
        .collect();
    let path = if rest.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", rest.join("&"))
    };
    (path, ed)
}

fn transport_json(t: &Transport) -> Result<Option<Value>, ConfigError> {
    Ok(match t {
        Transport::Tcp => None,
        Transport::Ws { path, host } => {
            let (path, ed) = split_early_data(path);
            let mut m = Map::new();
            m.insert("type".into(), json!("ws"));
            m.insert("path".into(), json!(path));
            if let Some(h) = host {
                m.insert("headers".into(), json!({ "Host": h }));
            }
            if let Some(ed) = ed {
                m.insert("max_early_data".into(), json!(ed));
                m.insert("early_data_header_name".into(), json!("Sec-WebSocket-Protocol"));
            }
            Some(Value::Object(m))
        }
        Transport::Grpc { service_name } => {
            Some(json!({ "type": "grpc", "service_name": service_name }))
        }
        Transport::HttpUpgrade { path, host } => {
            let mut m = Map::new();
            m.insert("type".into(), json!("httpupgrade"));
            m.insert("path".into(), json!(path));
            if let Some(h) = host {
                m.insert("host".into(), json!(h));
            }
            Some(Value::Object(m))
        }
        Transport::Http { path, host } => {
            let mut m = Map::new();
            m.insert("type".into(), json!("http"));
            m.insert("path".into(), json!(path));
            if let Some(h) = host {
                m.insert("host".into(), json!([h]));
            }
            Some(Value::Object(m))
        }
        Transport::Xhttp { .. } => return Err(ConfigError::NeedsXray("XHTTP")),
    })
}

pub fn outbound(node: &Node, tag: &str) -> Result<Value, ConfigError> {
    let mut m = Map::new();
    m.insert("type".into(), json!(node.proto_name()));
    m.insert("tag".into(), json!(tag));
    m.insert("server".into(), json!(node.server));
    m.insert("server_port".into(), json!(node.port));

    let mut quic = false;
    match &node.proto {
        Proto::Vless { uuid, flow, encryption } => {
            if encryption.is_some() {
                return Err(ConfigError::NeedsXray("шифрование VLESS"));
            }
            m.insert("uuid".into(), json!(uuid));
            if let Some(flow) = flow {
                m.insert("flow".into(), json!(flow));
            }
            m.insert("packet_encoding".into(), json!("xudp"));
        }
        Proto::Vmess { uuid, alter_id, security } => {
            m.insert("uuid".into(), json!(uuid));
            m.insert("alter_id".into(), json!(alter_id));
            m.insert("security".into(), json!(security));
        }
        Proto::Trojan { password } => {
            m.insert("password".into(), json!(password));
        }
        Proto::Shadowsocks { method, password } => {
            m.insert("method".into(), json!(method));
            m.insert("password".into(), json!(password));
        }
        Proto::Hysteria2 { password, obfs_password } => {
            quic = true;
            m.insert("password".into(), json!(password));
            if let Some(op) = obfs_password {
                m.insert("obfs".into(), json!({ "type": "salamander", "password": op }));
            }
        }
        Proto::Tuic { uuid, password, congestion_control } => {
            quic = true;
            m.insert("uuid".into(), json!(uuid));
            m.insert("password".into(), json!(password));
            if let Some(cc) = congestion_control {
                m.insert("congestion_control".into(), json!(cc));
            }
        }
    }

    match (&node.tls, quic) {
        (Some(tls), true) => {
            m.insert("tls".into(), quic_tls_json(tls));
        }
        (Some(tls), false) => {
            m.insert("tls".into(), tls_json(tls));
        }
        (None, true) => {
            return Err(ConfigError::Invalid(format!(
                "{} не работает без TLS",
                node.proto_name()
            )))
        }
        (None, false) => {}
    }

    if !quic {
        if let Some(t) = transport_json(&node.transport)? {
            m.insert("transport".into(), t);
        }
    }
    Ok(Value::Object(m))
}

fn clean(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|s| s.trim().trim_start_matches('.').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Конфиг, в котором sing-box сам подключается к серверу.
pub fn build_config(node: &Node, opts: &Options) -> Result<Value, ConfigError> {
    assemble(outbound(node, PROXY)?, &[], opts)
}

/// Конфиг для связки с xray: sing-box держит туннель и маршрутизацию,
/// а трафик отдаёт в локальный SOCKS-порт xray.
pub fn build_chained_config(
    socks_port: u16,
    core_process: &str,
    opts: &Options,
) -> Result<Value, ConfigError> {
    let proxy = json!({
        "type": "socks",
        "tag": PROXY,
        "server": "127.0.0.1",
        "server_port": socks_port,
        "version": "5"
    });
    assemble(proxy, &[core_process], opts)
}

/// `core_processes` — вспомогательные ядра: их собственный трафик обязан идти
/// напрямую, иначе в режиме TUN он зациклится сам на себя.
fn assemble(proxy: Value, core_processes: &[&str], opts: &Options) -> Result<Value, ConfigError> {
    let suffixes = clean(&opts.direct_suffixes);

    let (apps, apps_outbound, apps_dns, final_outbound, final_dns) = match &opts.split {
        Split::All => (Vec::new(), DIRECT, DNS_DIRECT, PROXY, DNS_REMOTE),
        Split::Except(a) => (clean(a), DIRECT, DNS_DIRECT, PROXY, DNS_REMOTE),
        Split::Only(a) => (clean(a), PROXY, DNS_REMOTE, DIRECT, DNS_DIRECT),
    };
    if apps.is_empty() && !matches!(opts.split, Split::All) {
        return Err(ConfigError::Invalid(
            "для раздельного туннелирования не выбрано ни одного приложения".into(),
        ));
    }

    let mut dns_rules = Vec::new();
    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_is_private": true, "outbound": DIRECT }),
    ];
    if !core_processes.is_empty() {
        dns_rules.push(json!({ "process_name": core_processes, "server": DNS_DIRECT }));
        route_rules.push(json!({ "process_name": core_processes, "outbound": DIRECT }));
    }
    if !suffixes.is_empty() {
        dns_rules.push(json!({ "domain_suffix": suffixes, "server": DNS_DIRECT }));
        route_rules.push(json!({ "domain_suffix": suffixes, "outbound": DIRECT }));
    }
    if !apps.is_empty() {
        dns_rules.push(json!({ "process_name": apps, "server": apps_dns }));
        route_rules.push(json!({ "process_name": apps, "outbound": apps_outbound }));
    }

    let mut inbound = match &opts.mode {
        Mode::Tun => json!({
            "type": "tun",
            "tag": "tun-in",
            "interface_name": "FlexConnect",
            "address": if opts.ipv6 { json!(["172.19.0.1/30", "fdfe:dcba:9876::1/126"]) } else { json!(["172.19.0.1/30"]) },
            "auto_route": true,
            "strict_route": true,
            "stack": "mixed"
        }),
        Mode::Proxy { port } => json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": port
        }),
    };

    if cfg!(target_os = "linux") && matches!(opts.mode, Mode::Tun) {
        inbound["auto_redirect"] = json!(true);
        inbound["dns_mode"] = json!("hijack");
    }

    Ok(json!({
        "log": { "level": opts.log_level, "timestamp": true },
        "dns": {
            "servers": [
                { "type": "https", "tag": DNS_REMOTE, "server": opts.remote_dns, "detour": PROXY },
                { "type": "udp", "tag": DNS_DIRECT, "server": opts.direct_dns }
            ],
            "rules": dns_rules,
            "final": final_dns,
            "strategy": if opts.ipv6 { "prefer_ipv4" } else { "ipv4_only" }
        },
        "inbounds": [inbound],
        "outbounds": [proxy, { "type": "direct", "tag": DIRECT }],
        "route": {
            "rules": route_rules,
            "final": final_outbound,
            "auto_detect_interface": true,
            "default_domain_resolver": DNS_DIRECT
        }
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tunnel_without_host_ipv6_is_ipv4_only() {
        let node = crate::link::parse_link("trojan://pw@example.com:443").unwrap();
        let with = super::build_config(&node, &super::Options::default()).unwrap();
        assert_eq!(with["inbounds"][0]["address"].as_array().unwrap().len(), 2);
        assert_eq!(with["dns"]["strategy"], "prefer_ipv4");
        let opts = super::Options { ipv6: false, ..Default::default() };
        let without = super::build_config(&node, &opts).unwrap();
        assert_eq!(without["inbounds"][0]["address"], serde_json::json!(["172.19.0.1/30"]));
        assert_eq!(without["dns"]["strategy"], "ipv4_only");
    }

    #[test]
    fn linux_tun_settings_do_not_leak_into_proxy_or_windows() {
        let node = crate::link::parse_link("trojan://pw@example.com:443").unwrap();
        let tun = super::build_config(&node, &super::Options::default()).unwrap();
        assert_eq!(tun["inbounds"][0]["auto_redirect"].as_bool(), if cfg!(target_os = "linux") { Some(true) } else { None });
        let opts = super::Options { mode: super::Mode::Proxy { port: 2080 }, ..Default::default() };
        let proxy = super::build_config(&node, &opts).unwrap();
        assert!(proxy["inbounds"][0]["auto_redirect"].is_null());
        assert!(proxy["inbounds"][0]["dns_mode"].is_null());
    }
    use super::*;
    use crate::link::parse_link;

    const REALITY: &str = "vless://uuid@example.com:443?security=reality&pbk=KEY&sid=ab&sni=www.microsoft.com&flow=xtls-rprx-vision#R";

    #[test]
    fn vless_reality_outbound() {
        let o = outbound(&parse_link(REALITY).unwrap(), "proxy").unwrap();
        assert_eq!(o["type"], "vless");
        assert_eq!(o["flow"], "xtls-rprx-vision");
        assert_eq!(o["tls"]["reality"]["public_key"], "KEY");
        // отпечаток в ссылке не задан — подставляется chrome
        assert_eq!(o["tls"]["utls"]["fingerprint"], "chrome");
        assert!(o.get("transport").is_none());
    }

    #[test]
    fn client_hello_fingerprint_is_normalized_for_saved_nodes() {
        let n = parse_link("vless://uuid@example.com:443?security=tls&fp=hellofirefox_120").unwrap();
        assert_eq!(outbound(&n, "proxy").unwrap()["tls"]["utls"]["fingerprint"], "firefox");
    }

    #[test]
    fn ws_early_data() {
        let n = parse_link("vless://uuid@example.com:443?type=ws&security=tls&path=%2Fws%3Fed%3D2048&host=h.example.com").unwrap();
        let o = outbound(&n, "proxy").unwrap();
        assert_eq!(o["transport"]["path"], "/ws");
        assert_eq!(o["transport"]["max_early_data"], 2048);
        assert_eq!(o["transport"]["headers"]["Host"], "h.example.com");
    }

    #[test]
    fn hysteria2_has_no_utls() {
        let n = parse_link("hy2://pw@example.com:443?sni=example.com&fp=chrome&obfs=salamander&obfs-password=op").unwrap();
        let o = outbound(&n, "proxy").unwrap();
        assert_eq!(o["obfs"]["password"], "op");
        assert!(o["tls"].get("utls").is_none());
    }

    #[test]
    fn xhttp_needs_xray() {
        let n = parse_link("vless://uuid@example.com:443?type=xhttp&security=tls").unwrap();
        assert_eq!(outbound(&n, "proxy"), Err(ConfigError::NeedsXray("XHTTP")));
    }

    #[test]
    fn default_config_routes_everything_to_proxy() {
        let c = build_config(&parse_link(REALITY).unwrap(), &Options::default()).unwrap();
        assert_eq!(c["inbounds"][0]["type"], "tun");
        assert_eq!(c["route"]["final"], "proxy");
        assert_eq!(c["dns"]["final"], "dns-remote");
        assert_eq!(c["route"]["rules"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn except_apps_go_direct() {
        let opts = Options {
            split: Split::Except(vec!["steam.exe".into()]),
            direct_suffixes: vec![".ru".into()],
            ..Options::default()
        };
        let c = build_config(&parse_link(REALITY).unwrap(), &opts).unwrap();
        let rules = c["route"]["rules"].as_array().unwrap();
        assert_eq!(rules[3]["domain_suffix"][0], "ru");
        assert_eq!(rules[4]["process_name"][0], "steam.exe");
        assert_eq!(rules[4]["outbound"], "direct");
        assert_eq!(c["route"]["final"], "proxy");
    }

    #[test]
    fn only_apps_go_to_proxy() {
        let opts = Options {
            mode: Mode::Proxy { port: 2080 },
            split: Split::Only(vec!["chrome.exe".into()]),
            ..Options::default()
        };
        let c = build_config(&parse_link(REALITY).unwrap(), &opts).unwrap();
        assert_eq!(c["inbounds"][0]["listen_port"], 2080);
        assert_eq!(c["route"]["rules"][3]["outbound"], "proxy");
        assert_eq!(c["route"]["final"], "direct");
        assert_eq!(c["dns"]["rules"][0]["server"], "dns-remote");
        assert_eq!(c["dns"]["final"], "dns-direct");
    }

    #[test]
    fn chained_config_keeps_xray_out_of_the_tunnel() {
        let opts = Options { split: Split::Only(vec!["chrome.exe".into()]), ..Options::default() };
        let c = build_chained_config(20808, "xray.exe", &opts).unwrap();
        assert_eq!(c["outbounds"][0]["type"], "socks");
        assert_eq!(c["outbounds"][0]["server_port"], 20808);
        // правило для xray стоит раньше пользовательских
        assert_eq!(c["route"]["rules"][3]["process_name"][0], "xray.exe");
        assert_eq!(c["route"]["rules"][3]["outbound"], "direct");
        assert_eq!(c["route"]["rules"][4]["process_name"][0], "chrome.exe");
        assert_eq!(c["dns"]["rules"][0]["server"], "dns-direct");
    }

    #[test]
    fn split_without_apps_is_an_error() {
        let opts = Options { split: Split::Only(vec![" ".into()]), ..Options::default() };
        assert!(build_config(&parse_link(REALITY).unwrap(), &opts).is_err());
    }
}
