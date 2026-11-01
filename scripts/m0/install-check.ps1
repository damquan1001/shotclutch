# Run only in a disposable CI runner/VM; installs and uninstalls the qualification MSI.
param([string] $Dist = 'dist/release', [ValidateSet('x64','arm64')][string] $Arch = 'x64',
  [string] $Fixtures, [string] $Output = 'target/ci/install', [switch] $DisposableEnvironment)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -and -not $DisposableEnvironment) { throw 'Use only in a disposable CI runner/VM; pass -DisposableEnvironment inside an explicitly disposable VM.' }
$out = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path $out | Out-Null
$summary = @{install_exit=$null; uninstall_exit=$null; result='failed'; arch=$Arch; clean_consumer_environment=$false}
$msis = @(Get-ChildItem -LiteralPath $Dist -Filter "*-windows-$Arch.msi")
if ($msis.Count -ne 1) { throw 'Expected exactly one matching MSI' }
$msi = $msis[0].FullName
$installed = $false
try {
  $installLog = Join-Path $out 'install.log'
  $p = Start-Process msiexec.exe -ArgumentList @('/i', "`"$msi`"", '/qn', '/norestart', '/l*v', "`"$installLog`"") -WindowStyle Hidden -Wait -PassThru
  $summary.install_exit = $p.ExitCode
  if ($p.ExitCode -ne 0) { throw "MSI installation failed: $($p.ExitCode); see install.log" }
  $installed = $true
  $association = Get-ItemProperty -LiteralPath 'Registry::HKEY_LOCAL_MACHINE\Software\Classes\.scproj'
  if ($association.'(default)' -ne 'ShotClutch.Project') { throw 'Installed native project association is incorrect' }
  $summary.native_project_association = '.scproj'
  $folder = Join-Path $env:ProgramFiles 'ShotClutch'
  $machine = if ($Arch -eq 'arm64') { 0xAA64 } else { 0x8664 }
  foreach ($name in 'shotclutch.exe','shotclutch-cli.exe') {
    $bytes = [IO.File]::ReadAllBytes((Join-Path $folder $name))
    $header = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt16($bytes, $header + 4) -ne $machine) { throw "Wrong executable architecture: $name" }
  }
  foreach ($name in 'NOTICE','ATTRIBUTION.md','LICENSE-MIT','LICENSE-APACHE') {
    if (-not (Test-Path -LiteralPath (Join-Path $folder "legal/$name"))) { throw "Missing installed notice: $name" }
  }
  $version = & (Join-Path $folder 'shotclutch-cli.exe') --version
  if ($LASTEXITCODE -ne 0 -or $version -notlike 'shotclutch-cli *') { throw 'Installed CLI did not run' }
  & (Join-Path $folder 'shotclutch-cli.exe') --demo inspect project
  if ($LASTEXITCODE -ne 0) { throw 'Installed CLI could not inspect a project' }
  $sentinel = Join-Path $folder 'user-created-project.scproj'
  Set-Content -LiteralPath $sentinel -Value 'qualification sentinel: preserve user-created files'
  if ($Fixtures) {
    & python scripts/ci/package-smoke.py exercise --fixtures $Fixtures --bin-dir $folder --output (Join-Path $out 'workflow')
    if ($LASTEXITCODE -ne 0) { throw 'Installed CLI edit/export workflow failed' }
  }
  $summary.msi_sha256 = (Get-FileHash -LiteralPath $msi).Hash
  $summary.version = $version
  $summary.installed_cli_sha256 = (Get-FileHash -LiteralPath (Join-Path $folder 'shotclutch-cli.exe')).Hash
  $summary.result = 'passed'
  Write-Output 'Installed package smoke passed. This does not qualify interactive playback on a clean machine.'
} finally {
  if ($installed) {
    $uninstallLog = Join-Path $out 'uninstall.log'
    $p = Start-Process msiexec.exe -ArgumentList @('/x', "`"$msi`"", '/qn', '/norestart', '/l*v', "`"$uninstallLog`"") -WindowStyle Hidden -Wait -PassThru
    $summary.uninstall_exit = $p.ExitCode
    $summary.result = 'failed'
    $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'summary.json')
    if ($p.ExitCode -ne 0) { throw "MSI uninstall failed: $($p.ExitCode)" }
    if (Test-Path -LiteralPath (Join-Path $env:ProgramFiles 'ShotClutch/shotclutch.exe')) { throw 'Uninstall left the product executable' }
    if ($sentinel -and -not (Test-Path -LiteralPath $sentinel)) { throw 'Uninstall removed a user-created file' }
    $summary.user_file_preserved = $true
    # Preserve a previous exercise failure even when cleanup succeeds.
    $summary.result = if ($summary.version) { 'passed' } else { 'failed' }
  }
  $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'summary.json')
}
