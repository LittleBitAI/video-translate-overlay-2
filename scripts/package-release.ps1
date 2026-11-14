param([ValidateSet('both', 'android', 'windows')][string]$Platform = 'both')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

function Run([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed ($LASTEXITCODE)" }
}

$package = Get-Content package.json -Raw | ConvertFrom-Json
$config = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$version = $package.version
if ($version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' -or $config.version -ne $version) { throw 'Package/Tauri versions must match and be stable' }
$cargoVersion = (Select-String -Path src-tauri/Cargo.toml -Pattern '^version = "([^"]+)"$').Matches[0].Groups[1].Value
if ($cargoVersion -ne $version) { throw 'Cargo version does not match' }
$notes = "docs/releases/$version.md"
if (!(Test-Path -LiteralPath $notes)) { throw "Missing feature notes: $notes" }
$notesText = Get-Content -LiteralPath $notes -Raw
if (!$notesText.Contains('<!-- vto-approved:v1 -->') -or !$notesText.Contains('## Features')) { throw 'Missing approval convention in prepared notes' }
$output = Join-Path $PWD "artifacts/releases/$version"
if ((Test-Path -LiteralPath $output) -and (Get-ChildItem -LiteralPath $output -Force | Select-Object -First 1)) {
    throw "Release output must be empty: $output. Archive the previous candidate before rebuilding; platform-only runs never reuse its artifacts."
}
New-Item -ItemType Directory -Force -Path $output | Out-Null

if ($Platform -in @('both', 'android')) {
    foreach ($name in @('JAVA_HOME', 'ANDROID_HOME', 'NDK_HOME')) {
        if (![Environment]::GetEnvironmentVariable($name)) { throw "$name is required" }
    }
    # The installed 0.1.6 APK uses this certificate. Changing it prevents an in-place upgrade.
    $certificate = 'fa6da2233c2062b48baf20b3562eacacce774da80f2e4bb3d20ec3a6a527ec28'
    $keyStore = if ($env:VTO_ANDROID_KEYSTORE) { $env:VTO_ANDROID_KEYSTORE } else { Join-Path $env:USERPROFILE '.android/debug.keystore' }
    $keyAlias = if ($env:VTO_ANDROID_KEY_ALIAS) { $env:VTO_ANDROID_KEY_ALIAS } else { 'androiddebugkey' }
    if (!$env:VTO_ANDROID_STORE_PASSWORD) { $env:VTO_ANDROID_STORE_PASSWORD = 'android' }
    if (!$env:VTO_ANDROID_KEY_PASSWORD) { $env:VTO_ANDROID_KEY_PASSWORD = $env:VTO_ANDROID_STORE_PASSWORD }
    if (!(Test-Path -LiteralPath $keyStore)) { throw 'The existing signing key is required; never generate a replacement' }
    $tools = Join-Path $env:ANDROID_HOME 'build-tools/36.0.0'
    $signer = Join-Path $tools 'apksigner.bat'
    $align = Join-Path $tools 'zipalign.exe'
    $buildLog = Join-Path $output 'android-build.log'
    & cargo tauri android build --apk --target aarch64 --features app 2>&1 | Tee-Object -FilePath $buildLog
    if ($LASTEXITCODE -ne 0) {
        # Windows without Developer Mode cannot create Tauri's JNI symlink.
        if (!(Select-String -LiteralPath $buildLog -Pattern 'symbolic|symlink|1314' -Quiet)) { throw "Android build failed; see $buildLog" }
        $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PWD 'target' }
        $jni = 'src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a'
        New-Item -ItemType Directory -Force -Path $jni | Out-Null
        Copy-Item -LiteralPath (Join-Path $targetRoot 'aarch64-linux-android/release/libvto.so') -Destination "$jni/libvto.so" -Force
        Copy-Item -LiteralPath (Join-Path $env:NDK_HOME 'toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so') -Destination "$jni/libc++_shared.so" -Force
        Push-Location src-tauri/gen/android
        try { Run './gradlew.bat' @('assembleArm64Release', '-x', 'rustBuildArm64Release') } finally { Pop-Location }
    }
    $unsigned = 'src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk'
    $aligned = Join-Path $output 'aligned.apk'
    Run $align @('-f', '-p', '4', $unsigned, $aligned)
    $apk = Join-Path $output "video-translate-overlay-$version-arm64.apk"
    Run $signer @('sign', '--ks', $keyStore, '--ks-key-alias', $keyAlias, '--ks-pass', 'env:VTO_ANDROID_STORE_PASSWORD', '--key-pass', 'env:VTO_ANDROID_KEY_PASSWORD', '--out', $apk, $aligned)
    $signature = & $signer verify --verbose --print-certs $apk
    if ($LASTEXITCODE -ne 0 -or ($signature -join "`n") -notmatch $certificate) { throw 'Signing continuity check failed' }
    $signature | Set-Content -LiteralPath (Join-Path $output 'android-signature.txt') -Encoding utf8NoBOM
    $badging = & (Join-Path $tools 'aapt.exe') dump badging $apk
    if ($LASTEXITCODE -ne 0 -or ($badging -join "`n") -notmatch "versionName='$([regex]::Escape($version))'" -or ($badging -join "`n") -notmatch "native-code: 'arm64-v8a'") { throw 'APK version or ABI does not match' }
    $badging | Set-Content -LiteralPath (Join-Path $output 'android-package.txt') -Encoding utf8NoBOM
    Remove-Item -LiteralPath $aligned
}

if ($Platform -in @('both', 'windows')) {
    Run 'cargo' @('tauri', 'build', '--features', 'app', '--bundles', 'nsis', '--target', 'x86_64-pc-windows-msvc', '--', '--no-default-features')
    $installer = "target/x86_64-pc-windows-msvc/release/bundle/nsis/video-translate-overlay_${version}_x64-setup.exe"
    Copy-Item -LiteralPath $installer -Destination (Join-Path $output "video-translate-overlay-$version-windows-x64-setup.exe") -Force
    if ((Get-Item -LiteralPath $installer).VersionInfo.ProductVersion -notlike "$version*") { throw 'Windows installer version does not match' }
}

Copy-Item -LiteralPath $notes -Destination (Join-Path $output 'release-notes.md') -Force
$hashes = Get-ChildItem -LiteralPath $output -File | Where-Object { $_.Extension -in @('.apk', '.exe') } | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
$hashes | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
$head = git rev-parse HEAD
$dirty = [bool](git status --porcelain)
@{version=$version; source_commit=$head; dirty=$dirty; notes_sha256=(Get-FileHash -LiteralPath $notes -Algorithm SHA256).Hash.ToLowerInvariant()} |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'manifest.json') -Encoding utf8NoBOM
Write-Output "Prepared $version ($Platform) in $output. Publication requires the user's approval."
