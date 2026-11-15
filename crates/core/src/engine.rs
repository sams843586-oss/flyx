//! Выбор ядра: один sing-box или связка sing-box + xray.

use serde_json::Value;

use crate::model::{Node, Proto, Transport};
use crate::singbox::{build_chained_config, build_config, ConfigError, Options};
use crate::xray;

/// Готовые конфиги для одного подключения.
pub struct Plan {
    pub singbox: Value,
    /// Заполнен, если сервер требует второго ядра.
    pub xray: Option<Value>,
}

/// Prefer Xray for VLESS interoperability. Xray no longer implements HTTP/2,
/// so that transport continues to use sing-box when supported there.
pub fn uses_xray(node: &Node) -> bool {
    node.needs_xray() || (matches!(&node.proto, Proto::Vless { .. }) && !matches!(&node.transport, Transport::Http { .. }))
}

/// Preserve compatibility with already installed 0.1 services: they select
/// Xray when the explicit VLESS encryption field is present. "none" is also
/// the correct Xray value for ordinary VLESS, without changing credentials.
pub fn service_node(mut node: Node) -> Node {
    if uses_xray(&node) {
        if let Proto::Vless { encryption, .. } = &mut node.proto {
            if encryption.is_none() { *encryption = Some("none".into()); }
        }
    }
    if let Some(tls) = &mut node.tls {
        if let Some(fp) = &mut tls.fingerprint { *fp = crate::fingerprint::normalize(fp).to_string(); }
    }
    node
}

/// `socks_port` — свободный локальный порт для связи sing-box → xray;
/// используется только если серверу нужен xray.
pub fn plan(node: &Node, opts: &Options, socks_port: u16) -> Result<Plan, ConfigError> {
    if uses_xray(node) {
        Ok(Plan {
            singbox: build_chained_config(socks_port, xray::PROCESS_NAME, opts)?,
            xray: Some(xray::build_config(node, socks_port, &opts.log_level)?),
        })
    } else {
        Ok(Plan {
            singbox: build_config(node, opts)?,
            xray: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::parse_link;
    #[test]
    fn ordinary_vless_uses_xray_and_a_local_chain() {
        let node = parse_link("vless://uuid@example.com:443?security=tls").unwrap();
        let config = plan(&node,&Options::default(),23000).unwrap();
        assert!(config.xray.is_some());
        assert_eq!(config.singbox["outbounds"][0]["type"],"socks");
        assert!(config.singbox["route"]["rules"].as_array().unwrap().iter().any(|rule|
            rule["process_name"].as_array().is_some_and(|names| names.iter().any(|n| n == crate::platform::XRAY))));
    }
    #[test]
    fn legacy_service_receives_explicit_vless_default() {
        let node = service_node(parse_link("vless://uuid@example.com:443?security=tls&fp=hellofirefox_120").unwrap());
        assert!(node.needs_xray());
        assert_eq!(crate::xray::outbound(&node).unwrap()["settings"]["vnext"][0]["users"][0]["encryption"],"none");
        assert_eq!(node.tls.unwrap().fingerprint.as_deref(),Some("firefox"));
    }
    #[test]
    fn http2_vless_remains_on_singbox() {
        let node = parse_link("vless://uuid@example.com:443?security=tls&type=h2").unwrap();
        assert!(!uses_xray(&node));
        assert!(plan(&node,&Options::default(),23000).unwrap().xray.is_none());
    }
}
