param([switch]$SkipTests)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    python tools/fetch-cores.py windows
    if ($LASTEXITCODE) { throw 'Core download failed' }
    # Tauri resolves bundled resources while compiling its build script, so
    # bootstrap helper binaries before building/testing the application.
    cargo build --release -p flexconnect-service -p flexconnect-cli
    if ($LASTEXITCODE) {throw 'Service bootstrap failed'}
    $taskMetadata=cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $taskRelease=Join-Path $taskMetadata.target_directory 'release'
    foreach($taskName in @('flexconnect-service.exe','flexconnect.exe')) {
        Copy-Item -LiteralPath (Join-Path $taskRelease $taskName) -Destination (Join-Path $taskRoot ('bin/'+$taskName)) -Force
    }
    if (!$SkipTests) { cargo test --workspace; if ($LASTEXITCODE) {throw 'Tests failed'} }
    cargo build --release --workspace
    if ($LASTEXITCODE) {throw 'Rust build failed'}
    $taskMetadata=cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $taskRelease=Join-Path $taskMetadata.target_directory 'release'
    foreach($taskName in @('flexconnect-service.exe','flexconnect.exe')) {
        Copy-Item -LiteralPath (Join-Path $taskRelease $taskName) -Destination (Join-Path $taskRoot ('bin/'+$taskName)) -Force
    }
    $env:FLYX_SERVICE_HELPER=Join-Path $taskRelease 'flexconnect-service.exe'
    Push-Location crates/app
    try {
        if ($env:FLYX_TAURI_CLI) { & $env:FLYX_TAURI_CLI build --bundles nsis }
        else { npx --yes @tauri-apps/cli@2.12.4 build --bundles nsis }
        if ($LASTEXITCODE) {throw 'Installer build failed'}
    } finally { Pop-Location }
    $taskOutput=Join-Path $taskRoot 'output/Release'
    New-Item -ItemType Directory -Force $taskOutput | Out-Null
    Get-ChildItem (Join-Path $taskRelease 'bundle/nsis') -File | Where-Object { $_.Name -match '\.(exe|sig)$' } | Copy-Item -Destination $taskOutput
} finally { Pop-Location }
