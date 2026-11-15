//! OS-specific names and per-user data locations, shared by UI and CLI.
use std::path::{Path, PathBuf};

pub const OS_NAME: &str = if cfg!(target_os = "linux") { "Linux" } else { "Windows" };
pub const SINGBOX: &str = if cfg!(windows) { "sing-box.exe" } else { "sing-box" };
pub const XRAY: &str = if cfg!(windows) { "xray.exe" } else { "xray" };

/// Есть ли у компьютера настоящий выход в интернет по IPv6.
///
/// Если его нет, туннелю нельзя объявлять IPv6-адрес: система решит, что IPv6
/// работает, программы пойдут на сайты по IPv6, и всё, что должно идти напрямую
/// (например, российские сайты), упрётся в «сеть недоступна».
pub fn host_has_ipv6() -> bool {
    use std::net::{Ipv6Addr, SocketAddr, UdpSocket};
    let Ok(socket) = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)) else { return false };
    // connect для UDP ничего не отправляет: он только спрашивает таблицу маршрутов
    let probe = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
    if socket.connect((probe, 53)).is_err() { return false; }
    match socket.local_addr() {
        Ok(SocketAddr::V6(local)) => is_global_ipv6(local.ip()),
        _ => false,
    }
}

/// Глобальный адрес (2000::/3). Адрес нашего же туннеля (fdfe:…) сюда не попадает,
/// поэтому ещё не снятый туннель прошлого подключения не выдаёт себя за IPv6-интернет.
fn is_global_ipv6(address: &std::net::Ipv6Addr) -> bool {
    address.segments()[0] & 0xe000 == 0x2000
}

pub fn data_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    { return std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("FlexConnect"))
        .ok_or_else(|| "не задана переменная APPDATA".into()); }
    #[cfg(not(windows))]
    { data_dir_from(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                    std::env::var_os("HOME").map(PathBuf::from)) }
}

#[cfg(any(not(windows), test))]
fn data_dir_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = xdg.filter(|p| p.is_absolute()) { return Ok(path.join("flyx")); }
    home.filter(|p| p.is_absolute()).map(|p| p.join(".config/flyx"))
        .ok_or_else(|| "не найдена домашняя папка пользователя".into())
}

/// Keys and subscriptions are readable only by their owner on Unix.
pub fn private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt;
      std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?; }
    Ok(())
}

pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt;
      file.set_permissions(std::fs::Permissions::from_mode(0o600))?; }
    file.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tunnel_and_local_addresses_are_not_ipv6_internet() {
        let parse = |s: &str| s.parse::<std::net::Ipv6Addr>().unwrap();
        assert!(is_global_ipv6(&parse("2a02:6b8::1d0")));
        assert!(!is_global_ipv6(&parse("fdfe:dcba:9876::1")));   // наш туннель
        assert!(!is_global_ipv6(&parse("fe80::1")));
        assert!(!is_global_ipv6(&parse("::1")));
    }
    #[test]
    fn xdg_directory_and_home_fallback() {
        // Absolute paths are native to the current test platform.
        let base = std::env::temp_dir();
        assert_eq!(data_dir_from(Some(base.clone()), None).unwrap(), base.join("flyx"));
        assert_eq!(data_dir_from(Some("relative".into()), Some(base.clone())).unwrap(), base.join(".config/flyx"));
        assert!(data_dir_from(None, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn configs_are_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("flyx-private-test-{}", uuid::Uuid::new_v4()));
        private_dir(&dir).unwrap();
        let file = dir.join("config.json");
        write_private(&file, b"{}").unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
