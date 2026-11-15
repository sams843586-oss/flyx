//! Convert ClientHello names used in share links to engine-supported families.
pub fn normalize(raw: &str) -> &str {
    let value = raw.trim();
    let lower = value.to_ascii_lowercase();
    let family = lower.strip_prefix("hello").unwrap_or(&lower);
    for name in ["randomized", "random", "firefox", "chrome", "safari", "edge", "ios", "android", "360", "qq"] {
        if family == name || family.strip_prefix(name).is_some_and(|s| s.starts_with('_') || s.starts_with('-')) {
            return name;
        }
    }
    // An unrecognized fingerprint must not prevent an otherwise valid node starting.
    "chrome"
}

#[cfg(test)]
mod tests {
    #[test]
    fn client_hello_aliases() {
        for (raw, expected) in [("hellofirefox_120", "firefox"), ("HelloChrome_120", "chrome"), (" EDGE ", "edge"), ("randomized", "randomized"), ("unknown", "chrome")] {
            assert_eq!(super::normalize(raw), expected);
        }
    }
}
