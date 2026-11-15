fn main() {
    println!("cargo:rerun-if-changed=windows.manifest");
    let mut windows = tauri_build::WindowsAttributes::new();
    // Release binaries request UAC elevation before any application code runs.
    // Keep development/test binaries runnable by the ordinary test runner.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("PROFILE").as_deref() == Ok("release") {
        windows = windows.app_manifest(include_str!("windows.manifest"));
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("не удалось собрать ресурсы приложения");
}
