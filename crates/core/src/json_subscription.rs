//! Import Xray subscription arrays as individual nodes; use our own routing.
use serde_json::Value;
use crate::model::*;
use crate::subscription::{Subscription, Skipped, is_provider_notice};

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}
fn required(v: &Value, key: &str) -> Result<String, String> {
    text(v, key).ok_or_else(|| format!("в JSON отсутствует {key}"))
}
fn port(v: &Value) -> Result<u16, String> {
    let n = v.as_u64().or_else(|| v.as_str()?.parse().ok()).unwrap_or(0);
    u16::try_from(n).ok().filter(|n| *n != 0).ok_or_else(|| "неверный порт в JSON".into())
}

fn transport(s: &Value) -> Result<Transport, String> {
    let kind = s.get("network").and_then(Value::as_str).unwrap_or("tcp");
    let obj = &s[match kind { "ws" => "wsSettings", "grpc" => "grpcSettings", "httpupgrade" => "httpupgradeSettings", "xhttp" | "splithttp" => "xhttpSettings", "h2" | "http" => "httpSettings", _ => "tcpSettings" }];
    let path = text(obj,"path").unwrap_or_else(|| "/".into());
    let host = text(obj,"host").or_else(|| text(&obj["headers"],"Host")).or_else(|| text(&obj["headers"],"host"))
        .or_else(|| obj["host"].as_array()?.first()?.as_str().map(str::to_owned));
    Ok(match kind {
        "tcp" | "raw" | "" => {
            if obj["header"]["type"].as_str().is_some_and(|t| t != "none") {
                return Err("TCP с маскировкой заголовков пока не поддерживается".into());
            }
            Transport::Tcp
        }
        "ws" => Transport::Ws { path, host },
        "grpc" => Transport::Grpc { service_name: text(obj,"serviceName").unwrap_or_default() },
        "httpupgrade" => Transport::HttpUpgrade { path, host },
        "h2" | "http" => Transport::Http { path, host },
        "xhttp" | "splithttp" => Transport::Xhttp { path, host, mode:text(obj,"mode"), extra:obj.get("extra").map(Value::to_string) },
        other => return Err(format!("транспорт JSON {other} пока не поддерживается")),
    })
}

fn tls(s: &Value) -> Result<Option<Tls>, String> {
    let security = s["security"].as_str().unwrap_or("none");
    if security == "none" || security.is_empty() { return Ok(None); }
    if !matches!(security,"tls"|"reality") { return Err(format!("защита JSON {security} пока не поддерживается")); }
    let obj = &s[if security == "reality" { "realitySettings" } else { "tlsSettings" }];
    if obj.get("echConfigList").is_some_and(|v| !v.is_null() && v.as_str() != Some("")) {
        return Err("JSON с ECH пока не поддерживается".into());
    }
    Ok(Some(Tls {
        sni:text(obj,"serverName"),
        alpn:obj["alpn"].as_array().map(|v|v.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
        insecure:obj["allowInsecure"].as_bool().unwrap_or(false),
        fingerprint:text(obj,"fingerprint"),
        reality:if security == "reality" { Some(Reality {
            public_key:text(obj,"publicKey").or_else(||text(obj,"password")).ok_or("Reality JSON без публичного ключа")?,
            short_id:text(obj,"shortId").unwrap_or_default(),
        }) } else { None },
    }))
}

fn outbound(o: &Value, title: Option<&str>) -> Result<Vec<Node>, String> {
    if o.get("proxySettings").is_some() || o["streamSettings"]["sockopt"].get("dialerProxy").is_some() {
        return Err("цепочка прокси из JSON пока не поддерживается".into());
    }
    let protocol = required(o,"protocol")?;
    let stream = &o["streamSettings"];
    let transport = transport(stream)?;
    let tls = tls(stream)?;
    let settings = &o["settings"];
    let servers = settings[if matches!(protocol.as_str(),"vless"|"vmess") { "vnext" } else { "servers" }]
        .as_array().ok_or("нет списка серверов в JSON")?;
    let mut nodes = Vec::new();
    for server in servers {
        let address = required(server,"address")?;
        let port = port(&server["port"])?;
        let name = title.map(str::to_owned).or_else(||text(o,"remarks")).or_else(||text(o,"tag")).unwrap_or_else(||format!("{address}:{port}"));
        let users = if matches!(protocol.as_str(),"vless"|"vmess") {
            server["users"].as_array().ok_or("нет учётных записей в JSON")?.clone()
        } else { vec![server.clone()] };
        for user in users {
            let proto = match protocol.as_str() {
                "vless" => Proto::Vless {
                    uuid:required(&user,"id")?, flow:text(&user,"flow"),
                    encryption:text(&user,"encryption").filter(|s|s!="none"),
                },
                "vmess" => Proto::Vmess {
                    uuid:required(&user,"id")?, alter_id:user["alterId"].as_u64().unwrap_or(0) as u32,
                    security:text(&user,"security").unwrap_or_else(||"auto".into()),
                },
                "trojan" => Proto::Trojan { password:required(&user,"password")? },
                "shadowsocks" => Proto::Shadowsocks { method:required(&user,"method")?, password:required(&user,"password")? },
                other => return Err(format!("протокол JSON {other} пока не поддерживается")),
            };
            nodes.push(Node { name:name.clone(), server:address.clone(), port, proto, transport:transport.clone(), tls:tls.clone() });
        }
    }
    Ok(nodes)
}

pub fn parse(text: &str) -> Subscription {
    let mut result = Subscription::default();
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => { result.skipped.push(Skipped { line:1, scheme:Some("json".into()), reason:"некорректный JSON подписки".into() }); return result; }
    };
    let configs = value.as_array().cloned().unwrap_or_else(||vec![value]);
    let mut index = 0;
    for config in configs {
        let title = text_field(&config);
        let Some(outbounds) = config["outbounds"].as_array() else {
            result.skipped.push(Skipped { line:index+1, scheme:Some("json".into()), reason:"ожидалась конфигурация Xray с outbounds".into() }); continue;
        };
        let mut variant = 0;
        for o in outbounds {
            let protocol = o["protocol"].as_str().unwrap_or("");
            if matches!(protocol,"freedom"|"blackhole"|"dns"|"loopback") { continue; }
            index += 1;
            if !matches!(protocol,"vless"|"vmess"|"trojan"|"shadowsocks") {
                result.skipped.push(Skipped { line:index, scheme:Some(protocol.into()), reason:format!("протокол JSON {} пока не поддерживается",if protocol.is_empty(){"неизвестного формата"}else{protocol}) }); continue;
            }
            variant += 1;
            let name = title.as_ref().map(|n|if variant == 1 { n.clone() } else { format!("{n} · вариант {variant}") });
            match outbound(o,name.as_deref()) {
                Ok(nodes) => for node in nodes {
                    if is_provider_notice(&node) {
                        result.skipped.push(Skipped { line:index, scheme:Some(protocol.into()), reason:"провайдер вернул служебное сообщение вместо сервера".into() });
                    } else { result.nodes.push(node); }
                },
                Err(reason) => result.skipped.push(Skipped { line:index, scheme:Some(protocol.into()), reason }),
            }
        }
    }
    result
}

fn text_field(config: &Value) -> Option<String> {
    text(config,"remarks").or_else(||text(config,"name")).or_else(||text(config,"ps"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn xray_array_preserves_reality_and_transports() {
        let body = json!([{"remarks":"🇳🇱 Test", "outbounds":[
            {"protocol":"vless","settings":{"vnext":[{"address":"test.example","port":"443","users":[{"id":"uuid","flow":"xtls-rprx-vision","encryption":"none"}]}]},"streamSettings":{"network":"tcp","security":"reality","realitySettings":{"publicKey":"KEY","shortId":"ab","serverName":"example.org","fingerprint":"hellofirefox_120"}}},
            {"protocol":"trojan","settings":{"servers":[{"address":"other.example","port":443,"password":"pw"}]},"streamSettings":{"network":"ws","security":"tls","wsSettings":{"path":"/ws","headers":{"Host":"host.example"}}}},
            {"protocol":"freedom"}
        ]}]);
        let s = parse(&body.to_string());
        assert_eq!(s.nodes.len(),2); assert!(s.skipped.is_empty());
        assert_eq!(s.nodes[0].tls.as_ref().unwrap().reality.as_ref().unwrap().public_key,"KEY");
        assert_eq!(s.nodes[1].name,"🇳🇱 Test · вариант 2");
        assert_eq!(s.nodes[1].transport,Transport::Ws { path:"/ws".into(),host:Some("host.example".into()) });
    }
    #[test]
    fn provider_notice_and_unsupported_protocol_are_not_nodes() {
        let body = json!({"remarks":"Данное приложение не поддерживается!","outbounds":[
            {"protocol":"vless","settings":{"vnext":[{"address":"127.0.0.1","port":443,"users":[{"id":"uuid"}]}]}},
            {"protocol":"hysteria"}
        ]});
        let s = parse(&body.to_string()); assert!(s.nodes.is_empty()); assert_eq!(s.skipped.len(),2);
    }
}
