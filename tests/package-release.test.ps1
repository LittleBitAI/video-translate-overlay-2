$ErrorActionPreference = 'Stop'
$originalLocation = Get-Location
$source = Join-Path $PSScriptRoot '../scripts/package-release.ps1'
$sandbox = Join-Path ([IO.Path]::GetTempPath()) ('vto-package-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path "$sandbox/scripts", "$sandbox/src-tauri", "$sandbox/docs/releases" -Force | Out-Null
try {
    Copy-Item -LiteralPath $source -Destination "$sandbox/scripts/package-release.ps1"
    '{"version":"0.1.7"}' | Set-Content "$sandbox/package.json" -Encoding utf8NoBOM
    '{"version":"0.1.7"}' | Set-Content "$sandbox/src-tauri/tauri.conf.json" -Encoding utf8NoBOM
    'version = "0.1.7"' | Set-Content "$sandbox/src-tauri/Cargo.toml" -Encoding utf8NoBOM
    "<!-- vto-approved:v1 -->`n## Features`nPrepared notes." | Set-Content "$sandbox/docs/releases/0.1.7.md" -Encoding utf8NoBOM
    $output = "$sandbox/artifacts/releases/0.1.7"
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    function cargo { throw 'Unexpected build invocation' }
    $cases = 0
    foreach ($platform in @('android', 'windows', 'both')) {
        foreach ($name in @('video-translate-overlay-0.1.7-arm64.apk', 'video-translate-overlay-0.1.7-windows-x64-setup.exe', 'manifest.json', 'SHA256SUMS.txt', 'release-notes.md', '.partial')) {
            $existing = Join-Path $output $name
            'previous candidate' | Set-Content -LiteralPath $existing -Encoding utf8NoBOM
            if ($name -eq '.partial') { [IO.File]::SetAttributes($existing, [IO.FileAttributes]::Hidden) }
            $before = (Get-FileHash -LiteralPath $existing).Hash
            $failure = $null
            try { & "$sandbox/scripts/package-release.ps1" -Platform $platform } catch { $failure = $_.Exception.Message }
            if ($failure -notlike 'Release output must be empty:*') { throw "nonempty_candidate_is_denied: $platform failed with $failure" }
            if ((Get-FileHash -LiteralPath $existing).Hash -ne $before -or @(Get-ChildItem -LiteralPath $output -Force).Count -ne 1) { throw 'nonempty_candidate_is_denied: modified existing evidence' }
            Remove-Item -LiteralPath $existing -Force
            $cases++
        }
    }
    Write-Output "nonempty_candidate_is_denied: PASS ($cases platform/content combinations; no builds or candidate mutations)"
    function cargo { throw 'Build boundary reached' }
    foreach ($state in @('empty', 'absent')) {
        if ($state -eq 'absent') { Remove-Item -LiteralPath $output }
        $failure = $null
        try { & "$sandbox/scripts/package-release.ps1" -Platform windows } catch { $failure = $_.Exception.Message }
        if ($failure -ne 'Build boundary reached' -or !(Test-Path -LiteralPath $output -PathType Container) -or @(Get-ChildItem -LiteralPath $output -Force).Count -ne 0) { throw "fresh_candidate_reaches_build: $state failed with $failure" }
    }
    Write-Output 'fresh_candidate_reaches_build: PASS (empty and absent output directories; build boundary stubbed)'
} finally {
    Set-Location $originalLocation
    $resolved = [IO.Path]::GetFullPath($sandbox)
    if (!$resolved.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase) -or !(Split-Path $resolved -Leaf).StartsWith('vto-package-test-')) { throw 'Unexpected test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
