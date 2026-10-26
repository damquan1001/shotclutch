# Run only in a disposable CI runner/VM; installs and uninstalls the qualification MSI.
param([string] $Dist = 'dist/release', [ValidateSet('x64','arm64')][string] $Arch = 'x64')
$ErrorActionPreference = 'Stop'
$msis = @(Get-ChildItem -LiteralPath $Dist -Filter "*-windows-$Arch.msi")
if ($msis.Count -ne 1) { throw 'Expected exactly one matching MSI' }
$msi = $msis[0].FullName
$installed = $false
try {
  $p = Start-Process msiexec.exe -ArgumentList @('/i', "`"$msi`"", '/qn', '/l*v', 'install.log') -WindowStyle Hidden -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "MSI installation failed: $($p.ExitCode); see install.log" }
  $installed = $true
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
  Write-Output 'Installed package smoke passed. This does not qualify interactive playback on a clean machine.'
} finally {
  if ($installed) {
    $p = Start-Process msiexec.exe -ArgumentList @('/x', "`"$msi`"", '/qn') -WindowStyle Hidden -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "MSI uninstall failed: $($p.ExitCode)" }
    if (Test-Path -LiteralPath (Join-Path $env:ProgramFiles 'ShotClutch/shotclutch.exe')) { throw 'Uninstall left the product executable' }
  }
}
