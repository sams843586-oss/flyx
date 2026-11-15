# Release process

The updater public key in `crates/app/tauri.conf.json` must match the private signing key. Never replace it for an ordinary release: installed clients trust the existing key. Keep a private offline backup. Android releases also require the same Android signing keystore to update an installed APK.

1. Update the workspace version, Tauri config, Android `versionName`/`versionCode` and UI version label together.
2. Run the tests and build the Windows installer (`tools/build-windows.ps1`). Set `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` externally. The NSIS installer is signed automatically by Tauri and produces `.exe.sig`.
3. Build Linux with `tools/build-linux.sh`. Copy the `.deb` to the release directory and sign the actual bytes: `tauri signer sign -k PRIVATE_KEY_PATH PACKAGE.deb`. The public updater key validates both Windows and Linux packages.
4. Build the Android release APK using the external keystore variables documented in `ANDROID.md`. Debug keystores are not production release keys.
5. Put one Windows `*setup.exe` and one Linux `.deb`, each with its `.sig`, in `output/Release`. Run `python tools/release-manifest.py --version VERSION`. This generates `latest.json` with exact artifact URLs and signatures.
6. Create tag `vVERSION`, publish its GitHub release and upload the installers, signatures, `latest.json`, APK and `SHA256SUMS`. Keep the release title/notes honest about Linux/Android test status.
7. Verify that `releases/latest/download/latest.json` resolves over HTTPS and the installed current version reports no update. Test a newer signed release on dedicated test machines before wider rollout.

Do not upload private keys, keystores, passwords, subscription profiles, local browser data, or raw project input archives. Source dependencies are pinned by Cargo.lock and core archive SHA256. Signatures are not Authenticode certificates; this project currently does not have an Authenticode publisher certificate.

Linux updates use polkit/dpkg and need an interactive authentication agent. Android installation remains a user action through the Android package installer.
