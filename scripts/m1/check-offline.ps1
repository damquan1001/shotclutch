<#
Run the existing native mixed-media journey on this PC with networking disconnected.
ConnectedDiagnostic validates the same runner but can never report an offline pass.
No adapters, firewall rules, machine preferences, or execution policies are changed.
#>
[CmdletBinding()]
param(
    [string]$EvidenceDirectory = '',
    [switch]$ConnectedDiagnostic
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
if (-not $EvidenceDirectory) {
    $EvidenceDirectory = Join-Path $repoRoot ('target/m1/offline-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + $PID)
}
$evidenceRoot = [System.IO.Path]::GetFullPath($EvidenceDirectory)
$workspacePrefix = $repoRoot.TrimEnd([char[]]'\/') + [System.IO.Path]::DirectorySeparatorChar
if (-not $evidenceRoot.StartsWith($workspacePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'EvidenceDirectory must be a new directory within this workspace.'
}
if (Test-Path -LiteralPath $evidenceRoot) { throw 'EvidenceDirectory already exists; choose a new directory.' }
New-Item -ItemType Directory -Path $evidenceRoot -ErrorAction Stop | Out-Null

function Get-ActiveInterfaces {
    # Be conservative: virtual adapters also count. Never infer offline from an HTTP failure.
    @([System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() |
        Where-Object {
            $_.OperationalStatus -eq [System.Net.NetworkInformation.OperationalStatus]::Up -and
            $_.NetworkInterfaceType -ne [System.Net.NetworkInformation.NetworkInterfaceType]::Loopback
        } | ForEach-Object { $_.NetworkInterfaceType.ToString() })
}

$receipt = [ordered]@{
    version = 1
    startedUtc = [DateTime]::UtcNow.ToString('o')
    mode = $(if ($ConnectedDiagnostic) { 'connectedDiagnostic' } else { 'offline' })
    status = 'running'
    secondPc = 'excluded from M1 by user decision 2026-11-04'
    networkEvidence = 'non-loopback interface state before and after; no packet-capture or OS sandbox claim'
}
$exitCode = 1
$environmentBefore = @{}
foreach ($name in @('CARGO_NET_OFFLINE', 'CARGO_INCREMENTAL', 'SHOTCLUTCH_REQUIRE_ORACLES', 'SHOTCLUTCH_FFMPEG', 'SHOTCLUTCH_FFPROBE')) {
    $environmentBefore[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location -LiteralPath $repoRoot
try {
    $receipt.activeInterfacesBefore = @(Get-ActiveInterfaces)
    $receipt.sourceRevision = (& git rev-parse HEAD | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the source revision.' }
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'Cannot check source state.' }
    $receipt.dirty = [bool]$dirty
    if (-not $ConnectedDiagnostic -and $receipt.activeInterfacesBefore.Count -gt 0) {
        $receipt.status = 'blocked-network-active'
        $exitCode = 2
        throw 'Disconnect active physical/virtual network adapters, then rerun in a new evidence directory. No adapters were changed.'
    }
    if (-not $ConnectedDiagnostic -and $receipt.dirty) { throw 'Offline qualification requires a clean committed source tree.' }
    foreach ($tool in @('cargo', 'ffmpeg', 'ffprobe')) {
        $null = Get-Command $tool -ErrorAction Stop
    }
    $receipt.oracles = @{}
    foreach ($tool in @('ffmpeg', 'ffprobe')) {
        $toolPath = (Get-Command $tool -CommandType Application -ErrorAction Stop).Source
        [Environment]::SetEnvironmentVariable(('SHOTCLUTCH_' + $tool.ToUpperInvariant()), $toolPath, 'Process')
        $version = & $toolPath -version
        if ($LASTEXITCODE -ne 0) { throw ($tool + ' did not report its version.') }
        $receipt.oracles[$tool] = @{
            version = @($version)[0]
            sha256 = (Get-FileHash -LiteralPath $toolPath -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $receipt.cargoLockSha256 = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
    [Environment]::SetEnvironmentVariable('CARGO_NET_OFFLINE', 'true', 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_INCREMENTAL', '0', 'Process')
    [Environment]::SetEnvironmentVariable('SHOTCLUTCH_REQUIRE_ORACLES', '1', 'Process')
    $test = 'workflows::m1_mixed_rate_nested_proxy_persistence_and_local_collection_journey'
    $testArgs = @('test', '--locked', '--offline', '-p', 'shotclutch-cli', '--test', 'it', $test, '--', '--exact', '--nocapture')
    $receipt.command = @('cargo') + $testArgs
    $logPath = Join-Path $evidenceRoot 'journey.log'
    # Capture both streams concurrently without Windows PowerShell treating normal Cargo
    # build progress on stderr as NativeCommandError. All arguments are fixed tokens above.
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = (Get-Command cargo -CommandType Application -ErrorAction Stop).Source
    $start.Arguments = $testArgs -join ' '
    $start.WorkingDirectory = $repoRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = New-Object System.Text.UTF8Encoding($false)
    $start.StandardErrorEncoding = New-Object System.Text.UTF8Encoding($false)
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    try {
        if (-not $process.Start()) { throw 'Cannot start the native journey.' }
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(600000)) {
            # Only this owned Cargo process and its descendants are terminated on timeout.
            & taskkill.exe /PID $process.Id /T /F *> (Join-Path $evidenceRoot 'timeout-cleanup.log')
            throw 'Native journey exceeded its ten-minute budget; no pass is recorded.'
        }
        $stdout.Result | Set-Content -LiteralPath $logPath -Encoding utf8
        $stderr.Result | Set-Content -LiteralPath (Join-Path $evidenceRoot 'build.log') -Encoding utf8
        $testExit = $process.ExitCode
    } finally { $process.Dispose() }
    $receipt.testExitCode = $testExit
    $receipt.activeInterfacesAfter = @(Get-ActiveInterfaces)
    $receipt.logSha256 = (Get-FileHash -LiteralPath $logPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $receipt.buildLogSha256 = (Get-FileHash -LiteralPath (Join-Path $evidenceRoot 'build.log') -Algorithm SHA256).Hash.ToLowerInvariant()
    $logText = Get-Content -LiteralPath $logPath -Raw
    Get-Content -LiteralPath $logPath -Tail 12
    if ($testExit -ne 0 -or $logText -notmatch 'test result: ok\. 1 passed; 0 failed; 0 ignored;') {
        throw 'The required single journey did not pass. Missing oracle tools/cases cannot count as a pass.'
    }
    if (-not $ConnectedDiagnostic -and $receipt.activeInterfacesAfter.Count -gt 0) {
        throw 'Network adapters became active; this run cannot qualify offline operation.'
    }
    $cliPath = Join-Path $repoRoot 'target/debug/shotclutch-cli.exe'
    $receipt.cliSha256 = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $privacy = & $cliPath --data-dir (Join-Path $evidenceRoot 'status-data') exec privacy.status
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read native processing status.' }
    $receipt.processing = $privacy | ConvertFrom-Json
    if ($receipt.processing.localOnly -ne $true -or $receipt.processing.builtInCloudProcessingAvailable -ne $false) {
        throw 'The native local-only processing profile is not active.'
    }
    $receipt.fixtures = @{}
    foreach ($name in @('phone_vfr_h264_aac.mp4', 'camera_long_gop_2audio.mkv')) {
        $fixturePath = Join-Path $repoRoot ('target/fixtures/cli/workflows/' + $name)
        $receipt.fixtures[$name] = @{
            bytes = (Get-Item -LiteralPath $fixturePath).Length
            sha256 = (Get-FileHash -LiteralPath $fixturePath -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $receipt.activeInterfacesAfter = @(Get-ActiveInterfaces)
    if (-not $ConnectedDiagnostic -and $receipt.activeInterfacesAfter.Count -gt 0) { throw 'Network became active before completion.' }
    $endRevision = (& git rev-parse HEAD | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot recheck source revision.' }
    $endDirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'Cannot recheck source state.' }
    $receipt.sourceChanged = $endRevision -ne $receipt.sourceRevision -or [bool]$endDirty
    if (-not $ConnectedDiagnostic -and $receipt.sourceChanged) { throw 'Source changed during offline qualification.' }
    $receipt.status = $(if ($ConnectedDiagnostic) { 'passed-connected-diagnostic' } else { 'passed-offline-journey' })
    $receipt.testsPassed = 1
    $exitCode = 0
} catch {
    if ($receipt.status -eq 'running') { $receipt.status = 'failed' }
    $receipt.error = $_.Exception.Message
    Write-Host $receipt.error
} finally {
    foreach ($name in $environmentBefore.Keys) {
        [Environment]::SetEnvironmentVariable($name, $environmentBefore[$name], 'Process')
    }
    Pop-Location
    $receipt.finishedUtc = [DateTime]::UtcNow.ToString('o')
    $receipt | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $evidenceRoot 'receipt.json') -Encoding utf8
    Write-Output ($receipt.status + ': ' + (Join-Path $evidenceRoot 'receipt.json'))
}
exit $exitCode
