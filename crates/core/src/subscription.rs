//! Разбор тела подписки: список ссылок, открытым текстом или в base64.

use crate::link::{b64_decode_lenient, parse_link, scheme_of};
use crate::model::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    /// Номер строки в теле подписки, с единицы.
    pub line: usize,
    /// Только схема, без самой ссылки: в ней лежат ключи.
    pub scheme: Option<String>,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct Subscription {
    pub nodes: Vec<Node>,
    pub skipped: Vec<Skipped>,
}

pub fn is_provider_notice(node: &Node) -> bool {
    let name = node.name.to_lowercase();
    ["приложение не поддерживается", "рекомендуемые приложения", "app is not supported", "application not supported", "unsupported client", "recommended applications"]
        .iter().any(|text|name.contains(text))
        || ((node.server == "127.0.0.1" || node.server == "0.0.0.0" || node.server == "::1") && name.starts_with("t.me/"))
}

fn decode_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.contains("://") {
        return trimmed.to_string();
    }
    b64_decode_lenient(trimmed)
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| trimmed.to_string())
}

pub fn parse_subscription(body: &str) -> Subscription {
    let text = decode_body(body);
    if text.trim_start().starts_with(['{','[']) {
        return crate::json_subscription::parse(&text);
    }
    let mut sub = Subscription::default();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let scheme = scheme_of(line);
        if line.starts_with("happ://routing") {
            sub.skipped.push(Skipped {
                line: i + 1,
                scheme,
                reason: "профиль маршрутизации Happ — появится на этапе 2".into(),
            });
            continue;
        }
        match parse_link(line) {
            Ok(node) if is_provider_notice(&node) => sub.skipped.push(Skipped {
                line:i+1, scheme, reason:"провайдер вернул служебное сообщение вместо сервера".into(),
            }),
            Ok(node) => sub.nodes.push(node),
            Err(e) => sub.skipped.push(Skipped {
                line: i + 1,
                scheme,
                reason: e.0,
            }),
        }
    }
    sub
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine;

    const BODY: &str = "vless://uuid@a.example.com:443?security=tls#A\n\
        \n\
        # комментарий\n\
        trojan://pw@b.example.com:443#B\n\
        happ://routing/onadd/e30=\n\
        wireguard://nope@c.example.com:51820\n";

    #[test]
    fn plain_text() {
        let s = parse_subscription(BODY);
        assert_eq!(s.nodes.len(), 2);
        assert_eq!(s.skipped.len(), 2);
        assert_eq!(s.skipped[0].line, 5);
        assert_eq!(s.skipped[1].scheme.as_deref(), Some("wireguard"));
    }

    #[test]
    fn base64_with_line_wraps() {
        let enc = STANDARD.encode(BODY);
        let wrapped: String = enc
            .as_bytes()
            .chunks(40)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n");
        assert_eq!(parse_subscription(&wrapped).nodes.len(), 2);
    }

    #[test]
    fn base64_urlsafe_no_padding() {
        let enc = URL_SAFE_NO_PAD.encode(BODY);
        assert_eq!(parse_subscription(&enc).nodes.len(), 2);
    }

    #[test]
    fn garbage_gives_no_nodes() {
        let s = parse_subscription("<html>403</html>");
        assert!(s.nodes.is_empty());
    }
}
