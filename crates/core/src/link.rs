//! Разбор ссылок вида `vless://`, `vmess://`, `trojan://`, `ss://`, `hysteria2://`, `tuic://`.

use std::collections::HashMap;
use std::fmt;

use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;
use percent_encoding::percent_decode_str;
use serde_json::Value;
use url::Url;

use crate::model::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(msg.into()))
}

const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Base64 в том виде, в каком его реально присылают: с паддингом и без,
/// обычный и url-safe алфавит, с переносами строк.
pub fn b64_decode_lenient(s: &str) -> Option<Vec<u8>> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    LENIENT.decode(cleaned).ok()
}

fn b64_decode_str(s: &str) -> Option<String> {
    String::from_utf8(b64_decode_lenient(s)?).ok()
}

fn pct(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

pub fn scheme_of(raw: &str) -> Option<String> {
    raw.trim()
        .split_once("://")
        .map(|(s, _)| s.to_ascii_lowercase())
}

pub fn parse_link(raw: &str) -> Result<Node, ParseError> {
    let raw = raw.trim();
    let Some(scheme) = scheme_of(raw) else {
        return err("это не ссылка: нет схемы вида vless://");
    };
    match scheme.as_str() {
        "vless" => parse_vless(raw),
        "vmess" => parse_vmess(raw),
        "trojan" => parse_trojan(raw),
        "ss" => parse_ss(raw),
        "hysteria2" | "hy2" => parse_hysteria2(raw),
        "tuic" => parse_tuic(raw),
        other => err(format!("схема {other}:// пока не поддерживается")),
    }
}

struct Parts {
    user: String,
    pass: Option<String>,
    host: String,
    port: u16,
    q: HashMap<String, String>,
    name: String,
}

fn split_url(raw: &str, default_port: Option<u16>) -> Result<Parts, ParseError> {
    let url = Url::parse(raw).map_err(|e| ParseError(format!("не удалось разобрать ссылку: {e}")))?;
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| ParseError("в ссылке нет адреса сервера".into()))?
        .trim_matches(|c| c == '[' || c == ']')
        .to_string();
    let port = url
        .port()
        .or(default_port)
        .ok_or_else(|| ParseError("в ссылке нет порта".into()))?;
    let q: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let name = url
        .fragment()
        .map(pct)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{host}:{port}"));
    Ok(Parts {
        user: pct(url.username()),
        pass: url.password().map(pct),
        host,
        port,
        q,
        name,
    })
}

fn get<'a>(q: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| q.get(*k))
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1") | Some("true") | Some("True"))
}

fn split_list(v: Option<&str>) -> Vec<String> {
    v.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn build_tls(q: &HashMap<String, String>, default_security: &str) -> Result<Option<Tls>, ParseError> {
    let security = get(q, &["security"]).unwrap_or(default_security);
    let base = || Tls {
        sni: get(q, &["sni", "peer"]).map(str::to_string),
        alpn: split_list(get(q, &["alpn"])),
        insecure: truthy(get(q, &["allowInsecure", "insecure", "allow_insecure"])),
        fingerprint: get(q, &["fp"]).map(str::to_string),
        reality: None,
    };
    match security {
        "none" => Ok(None),
        "tls" | "xtls" => Ok(Some(base())),
        "reality" => {
            let Some(pbk) = get(q, &["pbk"]) else {
                return err("reality без публичного ключа (pbk)");
            };
            let mut tls = base();
            tls.reality = Some(Reality {
                public_key: pbk.to_string(),
                short_id: get(q, &["sid"]).unwrap_or("").to_string(),
            });
            Ok(Some(tls))
        }
        other => err(format!("неизвестный тип защиты: {other}")),
    }
}

fn build_transport(
    kind: &str,
    header_type: Option<&str>,
    path: Option<&str>,
    host: Option<&str>,
    service_name: Option<&str>,
    mode: Option<&str>,
    extra: Option<&str>,
) -> Result<Transport, ParseError> {
    let path_or_root = || path.unwrap_or("/").to_string();
    let host = host.map(str::to_string);
    match kind {
        "tcp" | "raw" | "" => match header_type {
            None | Some("none") => Ok(Transport::Tcp),
            Some(h) => err(format!("tcp с маскировкой заголовка {h} не поддерживается")),
        },
        "ws" => Ok(Transport::Ws { path: path_or_root(), host }),
        "grpc" => Ok(Transport::Grpc {
            service_name: service_name.unwrap_or("").to_string(),
        }),
        "httpupgrade" => Ok(Transport::HttpUpgrade { path: path_or_root(), host }),
        "http" | "h2" => Ok(Transport::Http { path: path_or_root(), host }),
        "xhttp" | "splithttp" => Ok(Transport::Xhttp {
            path: path_or_root(),
            host,
            mode: mode.map(str::to_string),
            extra: extra.map(str::to_string),
        }),
        other => err(format!("транспорт {other} не поддерживается")),
    }
}

fn transport_from_query(q: &HashMap<String, String>) -> Result<Transport, ParseError> {
    build_transport(
        get(q, &["type"]).unwrap_or("tcp"),
        get(q, &["headerType"]),
        get(q, &["path"]),
        get(q, &["host"]),
        get(q, &["serviceName"]),
        get(q, &["mode"]),
        get(q, &["extra"]),
    )
}

fn parse_vless(raw: &str) -> Result<Node, ParseError> {
    let p = split_url(raw, None)?;
    if p.user.is_empty() {
        return err("в vless-ссылке нет UUID");
    }
    Ok(Node {
        tls: build_tls(&p.q, "none")?,
        transport: transport_from_query(&p.q)?,
        proto: Proto::Vless {
            uuid: p.user,
            flow: get(&p.q, &["flow"]).map(str::to_string),
            encryption: get(&p.q, &["encryption"])
                .filter(|e| *e != "none")
                .map(str::to_string),
        },
        name: p.name,
        server: p.host,
        port: p.port,
    })
}

fn parse_trojan(raw: &str) -> Result<Node, ParseError> {
    let p = split_url(raw, Some(443))?;
    if p.user.is_empty() {
        return err("в trojan-ссылке нет пароля");
    }
    Ok(Node {
        tls: build_tls(&p.q, "tls")?,
        transport: transport_from_query(&p.q)?,
        proto: Proto::Trojan { password: p.user },
        name: p.name,
        server: p.host,
        port: p.port,
    })
}

fn parse_hysteria2(raw: &str) -> Result<Node, ParseError> {
    let p = split_url(raw, Some(443))?;
    let password = match &p.pass {
        Some(pass) => format!("{}:{}", p.user, pass),
        None => p.user.clone(),
    };
    let obfs_password = match get(&p.q, &["obfs"]) {
        None | Some("none") => None,
        Some("salamander") => Some(get(&p.q, &["obfs-password"]).unwrap_or("").to_string()),
        Some(other) => return err(format!("обфускация {other} не поддерживается")),
    };
    Ok(Node {
        tls: build_tls(&p.q, "tls")?,
        transport: Transport::Tcp,
        proto: Proto::Hysteria2 { password, obfs_password },
        name: p.name,
        server: p.host,
        port: p.port,
    })
}

fn parse_tuic(raw: &str) -> Result<Node, ParseError> {
    let p = split_url(raw, Some(443))?;
    if p.user.is_empty() {
        return err("в tuic-ссылке нет UUID");
    }
    Ok(Node {
        tls: build_tls(&p.q, "tls")?,
        transport: Transport::Tcp,
        proto: Proto::Tuic {
            uuid: p.user,
            password: p.pass.unwrap_or_default(),
            congestion_control: get(&p.q, &["congestion_control", "congestion-control"])
                .map(str::to_string),
        },
        name: p.name,
        server: p.host,
        port: p.port,
    })
}

fn parse_host_port(s: &str) -> Result<(String, u16), ParseError> {
    let s = s.trim_end_matches('/');
    let Some((host, port)) = s.rsplit_once(':') else {
        return err("нет порта в адресе сервера");
    };
    let port: u16 = port
        .parse()
        .map_err(|_| ParseError("порт должен быть числом".into()))?;
    let host = host.trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return err("пустой адрес сервера");
    }
    Ok((host.to_string(), port))
}

fn parse_ss(raw: &str) -> Result<Node, ParseError> {
    let body = &raw[raw.find("://").map(|i| i + 3).unwrap_or(0)..];
    let (body, fragment) = match body.split_once('#') {
        Some((b, f)) => (b, Some(pct(f))),
        None => (body, None),
    };
    let (body, query) = match body.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (body, None),
    };
    if query.is_some_and(|q| q.contains("plugin=")) {
        return err("shadowsocks с плагинами не поддерживается");
    }

    // SIP002: userinfo@host:port, где userinfo — base64 или открытый текст.
    // Старый формат: всё целиком в base64.
    let (userinfo, hostport) = match body.rsplit_once('@') {
        Some((u, h)) => {
            let u = if u.contains(':') {
                pct(u)
            } else {
                b64_decode_str(u).ok_or_else(|| ParseError("не удалось раскодировать ss-ссылку".into()))?
            };
            (u, h.to_string())
        }
        None => {
            let decoded = b64_decode_str(body)
                .ok_or_else(|| ParseError("не удалось раскодировать ss-ссылку".into()))?;
            let Some((u, h)) = decoded.rsplit_once('@') else {
                return err("в ss-ссылке нет адреса сервера");
            };
            (u.to_string(), h.to_string())
        }
    };
    let Some((method, password)) = userinfo.split_once(':') else {
        return err("в ss-ссылке нет метода шифрования");
    };
    let (server, port) = parse_host_port(&hostport)?;
    Ok(Node {
        name: fragment
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("{server}:{port}")),
        server,
        port,
        proto: Proto::Shadowsocks {
            method: method.to_string(),
            password: password.to_string(),
        },
        transport: Transport::Tcp,
        tls: None,
    })
}

/// В vmess-JSON числа и строки взаимозаменяемы: порт бывает и `443`, и `"443"`.
fn jstr(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_vmess(raw: &str) -> Result<Node, ParseError> {
    let body = &raw[raw.find("://").map(|i| i + 3).unwrap_or(0)..];
    let Some(json) = b64_decode_str(body) else {
        return err("vmess-ссылка должна быть в base64");
    };
    let v: Value = serde_json::from_str(&json)
        .map_err(|_| ParseError("внутри vmess-ссылки не JSON".into()))?;

    let server = jstr(&v, "add").ok_or_else(|| ParseError("в vmess нет адреса сервера".into()))?;
    let port: u16 = jstr(&v, "port")
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| ParseError("в vmess нет порта".into()))?;
    let uuid = jstr(&v, "id").ok_or_else(|| ParseError("в vmess нет UUID".into()))?;
    let host = jstr(&v, "host");
    let path = jstr(&v, "path");
    let net = jstr(&v, "net").unwrap_or_else(|| "tcp".into());

    let transport = build_transport(
        &net,
        jstr(&v, "type").as_deref(),
        path.as_deref(),
        host.as_deref(),
        // для grpc имя сервиса лежит в path
        path.as_deref(),
        None,
        None,
    )?;

    let tls = match jstr(&v, "tls").as_deref() {
        Some("tls") => Some(Tls {
            sni: jstr(&v, "sni").or_else(|| host.clone()),
            alpn: split_list(jstr(&v, "alpn").as_deref()),
            insecure: truthy(jstr(&v, "allowInsecure").as_deref()),
            fingerprint: jstr(&v, "fp"),
            reality: None,
        }),
        _ => None,
    };

    Ok(Node {
        name: jstr(&v, "ps").unwrap_or_else(|| format!("{server}:{port}")),
        server,
        port,
        proto: Proto::Vmess {
            uuid,
            alter_id: jstr(&v, "aid").and_then(|a| a.parse().ok()).unwrap_or(0),
            security: jstr(&v, "scy").unwrap_or_else(|| "auto".into()),
        },
        transport,
        tls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn vless_reality() {
        let n = parse_link(
            "vless://11111111-2222-3333-4444-555555555555@example.com:443?type=tcp&security=reality&pbk=PUBKEY&sid=ab12&sni=www.microsoft.com&fp=chrome&flow=xtls-rprx-vision#%F0%9F%87%B3%F0%9F%87%B1%20Amsterdam",
        )
        .unwrap();
        assert_eq!(n.name, "🇳🇱 Amsterdam");
        assert_eq!(n.server, "example.com");
        assert_eq!(n.port, 443);
        assert_eq!(n.security_name(), "reality");
        assert_eq!(n.transport, Transport::Tcp);
        let tls = n.tls.unwrap();
        assert_eq!(tls.sni.as_deref(), Some("www.microsoft.com"));
        assert_eq!(tls.reality.unwrap().short_id, "ab12");
        assert_eq!(
            n.proto,
            Proto::Vless {
                uuid: "11111111-2222-3333-4444-555555555555".into(),
                flow: Some("xtls-rprx-vision".into()),
                encryption: None
            }
        );
    }

    #[test]
    fn vless_ws_tls() {
        let n = parse_link(
            "vless://uuid@1.2.3.4:8443?type=ws&security=tls&path=%2Fws%3Fed%3D2048&host=cdn.example.com&sni=cdn.example.com&alpn=h2,http/1.1",
        )
        .unwrap();
        assert_eq!(n.name, "1.2.3.4:8443");
        assert_eq!(
            n.transport,
            Transport::Ws {
                path: "/ws?ed=2048".into(),
                host: Some("cdn.example.com".into())
            }
        );
        assert_eq!(n.tls.unwrap().alpn, vec!["h2", "http/1.1"]);
    }

    #[test]
    fn vless_ipv6_and_xhttp() {
        let n = parse_link("vless://uuid@[2001:db8::1]:443?type=xhttp&security=tls&path=/x&mode=auto&encryption=none&extra=%7B%22xPaddingBytes%22%3A%22100-1000%22%7D#v6").unwrap();
        assert_eq!(n.server, "2001:db8::1");
        assert_eq!(n.xray_reason(), Some("XHTTP"));
        assert_eq!(
            n.transport,
            Transport::Xhttp {
                path: "/x".into(),
                host: None,
                mode: Some("auto".into()),
                extra: Some(r#"{"xPaddingBytes":"100-1000"}"#.into())
            }
        );
        assert!(matches!(n.proto, Proto::Vless { encryption: None, .. }));
    }

    #[test]
    fn reality_without_key_fails() {
        assert!(parse_link("vless://uuid@example.com:443?security=reality").is_err());
    }

    #[test]
    fn trojan_defaults_to_tls() {
        let n = parse_link("trojan://p%40ss@example.com:443?sni=example.com#T").unwrap();
        assert_eq!(n.proto, Proto::Trojan { password: "p@ss".into() });
        assert_eq!(n.security_name(), "tls");
    }

    #[test]
    fn vmess_json() {
        let json = r#"{"v":"2","ps":"VM","add":"example.com","port":"443","id":"uuid","aid":0,"scy":"auto","net":"ws","host":"h.example.com","path":"/p","tls":"tls"}"#;
        let n = parse_link(&format!("vmess://{}", STANDARD.encode(json))).unwrap();
        assert_eq!(n.name, "VM");
        assert_eq!(n.port, 443);
        assert_eq!(
            n.transport,
            Transport::Ws { path: "/p".into(), host: Some("h.example.com".into()) }
        );
        assert_eq!(n.tls.unwrap().sni.as_deref(), Some("h.example.com"));
    }

    #[test]
    fn ss_sip002_base64_userinfo() {
        let user = STANDARD.encode("aes-256-gcm:secret");
        let n = parse_link(&format!("ss://{user}@example.com:8388#SS%20one")).unwrap();
        assert_eq!(n.name, "SS one");
        assert_eq!(
            n.proto,
            Proto::Shadowsocks { method: "aes-256-gcm".into(), password: "secret".into() }
        );
    }

    #[test]
    fn ss_legacy_fully_encoded() {
        let all = STANDARD.encode("chacha20-ietf-poly1305:pw@1.2.3.4:1234");
        let n = parse_link(&format!("ss://{all}#old")).unwrap();
        assert_eq!(n.server, "1.2.3.4");
        assert_eq!(n.port, 1234);
    }

    #[test]
    fn ss_plain_userinfo() {
        let n = parse_link("ss://2022-blake3-aes-128-gcm:a2V5@example.com:443").unwrap();
        assert_eq!(
            n.proto,
            Proto::Shadowsocks { method: "2022-blake3-aes-128-gcm".into(), password: "a2V5".into() }
        );
    }

    #[test]
    fn hysteria2_with_obfs() {
        let n = parse_link("hy2://pass@example.com:8443?sni=example.com&insecure=1&obfs=salamander&obfs-password=op#H").unwrap();
        assert_eq!(
            n.proto,
            Proto::Hysteria2 { password: "pass".into(), obfs_password: Some("op".into()) }
        );
        assert!(n.tls.unwrap().insecure);
    }

    #[test]
    fn tuic() {
        let n = parse_link("tuic://uuid:pw@example.com:443?congestion_control=bbr&alpn=h3&sni=example.com#Tu").unwrap();
        assert_eq!(
            n.proto,
            Proto::Tuic { uuid: "uuid".into(), password: "pw".into(), congestion_control: Some("bbr".into()) }
        );
    }

    #[test]
    fn unknown_scheme() {
        assert!(parse_link("wireguard://x@y:1").is_err());
        assert!(parse_link("просто текст").is_err());
    }
}
