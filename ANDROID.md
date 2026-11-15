# Android — experimental

First Android build: **Android 10+ (API 29), arm64-v8a**. The APK is signed with a dedicated project release key. This is an alpha: compiling and checking the shared parser does not replace testing on an actual phone.

The app uses the same UI, themes, nine languages, subscriptions, domain routing and mascot animations as desktop. The subscription parser/config generator is the existing Rust core compiled to WebAssembly. Native Android `VpnService` owns the TUN descriptor; `hev-socks5-tunnel` forwards traffic to local sing-box/Xray processes. Android's VPN permission is required. Root is not used.

## Installation

Install the APK from the GitHub release, open Flyx, add your own subscription or key, and press Connect. Approve Android's VPN dialog. Keep the foreground VPN notification enabled so you can return to the app or disconnect. Only one VPN can be active on Android at a time.

For per-app rules enter Android package IDs such as `org.telegram.messenger`, not Windows process names. Flyx itself is excluded to prevent the local proxy from routing its own sockets into the tunnel. The desktop proxy/TUN switch and tray option are hidden on Android.

On Android the Updates button opens the GitHub release page; APK installation remains a system-mediated user action. Windows/Linux signed in-app installation is separate. There is no root service or desktop autostart on Android. Always-on / lockdown VPN is not advertised by this alpha.

## Build

Requirements: JDK 17+, Android SDK 34, Gradle 8.9, Rust with `wasm32-unknown-unknown`, wasm-bindgen CLI **0.2.130**, Python 3. No NDK is needed; the pinned native executables and tunnel AAR are verified downloads from their original maintainers.

```sh
python3 tools/fetch-cores.py android
rustup target add wasm32-unknown-unknown
cargo build -p flyx-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/flyx_wasm.wasm --target web --out-dir ui/wasm
gradle -p android assembleDebug
```

The Rust target directory may be overridden by `CARGO_TARGET_DIR`. For a release APK set `FLYX_ANDROID_KEYSTORE`, `FLYX_ANDROID_STORE_PASSWORD` and `FLYX_ANDROID_KEY_PASSWORD`; the key alias is `flyx`. Keystores/passwords must stay outside the repository. Release command: `gradle -p android assembleRelease`.

Check before promoting from alpha: real subscription import, TUN connectivity and DNS, server switching, IPv6/UDP, per-app routing, screen-off/background operation, revoking VPN permission, and recovery after core/process termination. Connection logs are available inside the app; they are not uploaded automatically.
