use std::sync::Mutex;
use serde::Serialize;
use tauri::{Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub type PendingUpdate = Mutex<Option<Update>>;

#[derive(Serialize)]
pub struct Info { version: String, notes: String }

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle, pending: State<'_, PendingUpdate>) -> Result<Option<Info>, String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|update| Info { version: update.version.clone(), notes: update.body.clone().unwrap_or_default() });
    *pending.lock().map_err(|e| e.to_string())? = update;
    Ok(info)
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let update = pending.lock().map_err(|e| e.to_string())?.take().ok_or("Сначала проверьте обновления")?;
    let handle = app.clone();
    let mut downloaded = 0u64;
    let bytes = update.download(move |chunk, total| {
        downloaded += chunk as u64;
        let _ = handle.emit("update-progress", serde_json::json!({"downloaded":downloaded,"total":total}));
    }, || {}).await.map_err(|e| e.to_string())?;
    // Complete and verify the download before interrupting the user's tunnel.
    let _ = app.emit("update-installing", ());
    #[cfg(target_os = "linux")]
    {
        // Tauri's Linux installer handles AppImage; Flyx ships a signed .deb.
        // Verify with the same updater key, then ask polkit to install the package.
        let dir = flexconnect_core::platform::data_dir()?.join("updates");
        flexconnect_core::platform::private_dir(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("flyx-update.deb");
        flexconnect_core::platform::write_private(&path, &bytes).map_err(|e| e.to_string())?;
        let output = std::process::Command::new("pkexec").args(["/usr/bin/dpkg", "-i"])
            .arg(&path).output().map_err(|e| e.to_string())?;
        std::fs::remove_file(path).ok();
        if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).into_owned()); }
        std::process::Command::new("/usr/bin/flyx").spawn().map_err(|e| e.to_string())?;
        app.exit(0);
        return Ok(());
    }
    #[cfg(windows)]
    {
    if let Ok(status) = super::ask(&flexconnect_core::ipc::Request::Status) {
        if status.state == flexconnect_core::ipc::State::Connected {
            let response = super::ask(&flexconnect_core::ipc::Request::Disconnect)?;
            if !response.ok { return Err(response.error.unwrap_or_else(|| "Не удалось отключить VPN".into())); }
        }
    }
        update.install(bytes).map_err(|e| e.to_string())
    }
}
