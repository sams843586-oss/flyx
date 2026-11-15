//! A persistent random installation ID for providers which bind subscriptions
//! to a device. No hardware serial numbers or other clients' IDs are used.
use std::io::Write;

pub fn subscription_id() -> Result<String, String> {
    let dir = crate::platform::data_dir()?;
    let path = dir.join("device-id.txt");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim();
        if (10..=64).contains(&id.len()) && id.chars().all(|c|c.is_ascii_alphanumeric() || c == '-' || c == '=') {
            return Ok(id.to_string());
        }
        return Err("неверный идентификатор устройства в данных приложения".into());
    }
    crate::platform::private_dir(&dir).map_err(|e|e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    match options.open(&path) {
        Ok(mut file) => { file.write_all(id.as_bytes()).map_err(|e|e.to_string())?; Ok(id) },
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => std::fs::read_to_string(path).map(|id|id.trim().to_string()).map_err(|e|e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}
