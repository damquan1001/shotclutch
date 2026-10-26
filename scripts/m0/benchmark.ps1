param(
  [Parameter(Mandatory)][string] $Checkout,
  [Parameter(Mandatory)][string] $Label,
  [Parameter(Mandatory)][string] $Revision,
  [string] $Output = 'target/m0/measurements',
  [ValidateRange(1,100)][int] $Repeat = 3,
  [switch] $Quick
)
$ErrorActionPreference = 'Stop'
if ($Label -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Label must be a filename-safe identifier' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$checkoutPath = (Resolve-Path -LiteralPath $Checkout).Path
$out = [IO.Path]::GetFullPath((Join-Path $Output $Label))
New-Item -ItemType Directory -Force -Path $out | Out-Null
# Both products read the same fixtures. Restore caller environment when finished.
$previous = @{}
foreach ($key in 'FILMCRAFT_FIXTURES','SHOTCLUTCH_FIXTURES') {
  $previous[$key] = [Environment]::GetEnvironmentVariable($key)
  [Environment]::SetEnvironmentVariable($key, (Join-Path $root 'target/fixtures'))
}
Push-Location -LiteralPath $checkoutPath
try {
  & cargo build --locked --release -p filmcraft-ui-egui --example bench
  if ($LASTEXITCODE -ne 0) { throw 'Benchmark build failed' }
  $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $checkoutPath 'target' }
  $exe = Join-Path $targetRoot 'release/examples/bench.exe'
  $results = @()
  foreach ($section in 'decode','seek','playback','scrub','timeline','export','project') {
    $json = Join-Path $out "$section.json"
    $args = @('--section',$section,'--json',$json,'--repeat',"$Repeat",'--hw','off','--cpu')
    if ($Quick) { $args += '--quick' }
    # Generate missing fixtures and warm each workload outside the measured interval.
    $warmArgs = @('--section',$section,'--json',(Join-Path $out "$section.warmup.json"),'--repeat','1','--hw','off','--cpu','--quick')
    & $exe @warmArgs > (Join-Path $out "$section.warmup.stdout.log") 2> (Join-Path $out "$section.warmup.stderr.log")
    if ($LASTEXITCODE -ne 0) { throw "Benchmark preparation failed: $section" }
    # Argument values with spaces must remain single Windows arguments.
    $quoted = $args | ForEach-Object { '"' + $_.Replace('"','\"') + '"' }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $exe -ArgumentList $quoted -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $out "$section.stdout.log") -RedirectStandardError (Join-Path $out "$section.stderr.log")
    $peakBytes = 0L
    while (-not $process.WaitForExit(100)) {
      $process.Refresh()
      $peakBytes = [Math]::Max($peakBytes, $process.PeakWorkingSet64)
    }
    $process.WaitForExit()
    $watch.Stop()
    $result = @{section=$section; exit_code=$process.ExitCode; wall_ms=$watch.Elapsed.TotalMilliseconds; peak_working_set_bytes=$(if ($peakBytes -gt 0) {$peakBytes} else {$null}); report="$section.json"}
    $results += $result
    if ($process.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $json)) { throw "Benchmark section failed: $section" }
  }
  $hardware = @{cpu=$env:PROCESSOR_IDENTIFIER; logical_cores=[Environment]::ProcessorCount; os=[Environment]::OSVersion.VersionString; gpu=$null; memory_bytes=$null}
  try {
    $hardware.gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,AdapterRAM,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate)
    $hardware.cpu_details = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed)
    $hardware.storage = @(Get-CimInstance Win32_DiskDrive | Select-Object Model,InterfaceType,Size)
    $hardware.power_scheme = (& powercfg /getactivescheme | Out-String).Trim()
    $hardware.memory_bytes = [double](Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize * 1024
  } catch { $hardware.unavailable = $_.Exception.Message }
  $fixtureRoot = Join-Path $root 'target/fixtures/playback'
  $fixtures = @(Get-ChildItem -LiteralPath $fixtureRoot -File -Recurse | ForEach-Object { @{path=$_.FullName.Substring($fixtureRoot.Length+1); bytes=$_.Length; sha256=(Get-FileHash -LiteralPath $_.FullName).Hash} })
  @{label=$Label; revision=$Revision; checkout=$checkoutPath; collected_utc=[DateTime]::UtcNow.ToString('o'); rustc=(& rustc --version); cargo=(& cargo --version); cargo_lock_sha256=(Get-FileHash -LiteralPath (Join-Path $checkoutPath 'Cargo.lock')).Hash; ffmpeg=(& ffmpeg -version | Select-Object -First 1); ffprobe=(& ffprobe -version | Select-Object -First 1); hardware=$hardware; settings=@{repeat=$Repeat; quick=[bool]$Quick; decoder='software'; compositor='CPU'; cache='unmeasured quick preparation per section; measured fresh child; OS cache uncontrolled; case-specific caches in raw report'; ceilings='host defaults'; inherited_load_caveat='upstream Windows empty load is unavailable, not an idle-machine measurement'}; fixtures=$fixtures; sections=$results} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $out 'manifest.json')
} finally {
  Pop-Location
  foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key]) }
}
Write-Output "Benchmark evidence: $out"
