<#
.SYNOPSIS
  Build, sign and package ShotClutch for Windows.

.DESCRIPTION
  Produces, in $env:DIST (default: dist/release):
    shotclutch-<version>-windows-<arch>.msi            per-machine installer (WiX v5)
    shotclutch-<version>-windows-<arch>-portable.zip   shotclutch.exe + shotclutch-cli.exe + portable.txt

  The binaries link the C runtime statically (+crt-static), so neither the MSI nor the portable
  zip needs the Visual C++ redistributable. Signing is delegated to sign.ps1 (skipped with a
  warning when no signing secrets are set). Pass -Sign explicitly; qualification builds are unsigned by default.

  Needs: Rust (MSVC toolchain + the target), the Windows SDK (rc.exe, signtool.exe),
  and WiX v5: dotnet tool install --global wix --version 5.0.2

.EXAMPLE
  pwsh packaging/windows/package.ps1 -Arch x64
  pwsh packaging/windows/package.ps1 -Arch x86 -SkipBuild
  pwsh packaging/windows/package.ps1 -Arch arm64     # cross-compiled; needs the MSVC ARM64 build tools
#>
param(
  [ValidateSet('x64', 'x86', 'arm64')] [string] $Arch = 'x64',
  [switch] $SkipBuild,
  [switch] $Sign
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Wix = Join-Path $Root 'target\m0\tools\wix.exe'
if (-not (Test-Path -LiteralPath $Wix)) { $Wix = (Get-Command wix -ErrorAction Stop).Source }
$WixVersion = & $Wix --version
if ($LASTEXITCODE -ne 0 -or $WixVersion -notlike '5.0.2*') { throw 'WiX 5.0.2 required. Run: dotnet tool install wix --tool-path target/m0/tools --version 5.0.2' }


function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# The version lives in one place: [workspace.package] version in the root Cargo.toml.
$Version = if ($env:SHOTCLUTCH_VERSION) { $env:SHOTCLUTCH_VERSION } else { $env:FILMCRAFT_VERSION }
if (-not $Version) {
  $inPkg = $false
  foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
    if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
  }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
# MSI ProductVersion is numeric (major.minor.build); pre-release tags are dropped there.
$MsiVersion = ($Version -split '-')[0]

$Target = switch ($Arch) { 'x64' { 'x86_64-pc-windows-msvc' } 'x86' { 'i686-pc-windows-msvc' } 'arm64' { 'aarch64-pc-windows-msvc' } }
$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

if (-not $env:SHOTCLUTCH_BUILD_SHA) { $env:SHOTCLUTCH_BUILD_SHA = if ($env:FILMCRAFT_BUILD_SHA) { $env:FILMCRAFT_BUILD_SHA } else { (git -C $Root rev-parse HEAD 2>$null) } }
if (-not $env:SHOTCLUTCH_BUILD_DATE) { $env:SHOTCLUTCH_BUILD_DATE = if ($env:FILMCRAFT_BUILD_DATE) { $env:FILMCRAFT_BUILD_DATE } else { (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd') } }

Write-Output "ShotClutch $Version for Windows $Arch ($Target)"

if (-not $SkipBuild) {
  # Static CRT: no VC++ redistributable needed. Scoped to the target so host build scripts and
  # proc-macros are unaffected.
  $flagVar = 'CARGO_TARGET_' + ($Target.ToUpper() -replace '-', '_') + '_RUSTFLAGS'
  [Environment]::SetEnvironmentVariable($flagVar, '-C target-feature=+crt-static')
  # Fail the build (rather than warn) if the icon/VERSIONINFO can't be embedded.
  $env:SHOTCLUTCH_REQUIRE_WINRES = '1'
  Invoke-Native "cargo build ($Target)" { cargo build --release --locked -p filmcraft -p filmcraft-cli --target $Target }
}

$Bin = Join-Path $TargetDir "$Target\release"

# Check both binaries' PE headers before packaging. Machine (COFF header) must match -Arch, so an
# x64 build can never ship labelled arm64. Subsystem (optional header): 2 = Windows GUI, 3 = console.
# The app must be GUI (no console window opens with it); the CLI must stay console so its output
# reaches the terminal.
function Get-PeHeader([string] $Path) {
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  $pe = [BitConverter]::ToInt32($bytes, 0x3C)
  return @{ Machine = [BitConverter]::ToUInt16($bytes, $pe + 4); Subsystem = [BitConverter]::ToUInt16($bytes, $pe + 0x5C) }
}
$Machine = switch ($Arch) { 'x64' { 0x8664 } 'x86' { 0x14C } 'arm64' { 0xAA64 } }
foreach ($check in @(@('shotclutch.exe', 2), @('shotclutch-cli.exe', 3))) {
  $h = Get-PeHeader (Join-Path $Bin $check[0])
  if ($h.Machine -ne $Machine) { throw "$($check[0]) is for machine 0x$('{0:X}' -f $h.Machine), expected 0x$('{0:X}' -f $Machine) ($Arch)" }
  if ($h.Subsystem -ne $check[1]) { throw "$($check[0]) has PE subsystem $($h.Subsystem), expected $($check[1])" }
  Write-Output "ok $($check[0]): $Arch, PE subsystem $($h.Subsystem)"
}
function Assert-OwnedPath([string] $Path, [string] $OwnerRoot) {
  $absolute = [IO.Path]::GetFullPath($Path)
  $rootPath = [IO.Path]::GetFullPath($OwnerRoot).TrimEnd('\') + '\'
  if (-not $absolute.StartsWith($rootPath, [StringComparison]::OrdinalIgnoreCase)) { throw "Unsafe package cleanup target: $absolute" }
}
$Stage = Join-Path $TargetDir "windows-package\$Arch"
Assert-OwnedPath $Stage $TargetDir
Remove-Item -Recurse -Force -LiteralPath $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item (Join-Path $Bin 'shotclutch.exe'), (Join-Path $Bin 'shotclutch-cli.exe') $Stage
$Legal = Join-Path $Stage 'legal'
New-Item -ItemType Directory -Force -Path $Legal | Out-Null
foreach ($file in 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md') { Copy-Item -LiteralPath (Join-Path $Root $file) -Destination $Legal }
Copy-Item (Join-Path $Root 'assets\fonts\OFL-*.txt') $Legal
@{
  product = 'ShotClutch'; version = $Version; target = $Target
  commit = $env:SHOTCLUTCH_BUILD_SHA; build_date_utc = $env:SHOTCLUTCH_BUILD_DATE
  observed_commit = (git -C $Root rev-parse HEAD)
  dirty = [bool](git -C $Root status --porcelain)
  rustc = (& rustc --version); cargo_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $Root 'Cargo.lock')).Hash
  signing_requested = [bool]$Sign; wix = $WixVersion
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Legal 'build-info.json') -Encoding utf8

if ($env:CRAFT_FONTS_DIR) {
  foreach ($lic in Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt')) { Copy-Item $lic.FullName (Join-Path $Legal "OFL-$($lic.Directory.Name).txt") }
}
# Generate the MSI legal component list from the same directory copied into the portable ZIP.
$LegalWxs = Join-Path $Stage 'legal.wxs'
$entries = @()
$i = 0
foreach ($file in Get-ChildItem -LiteralPath $Legal -File | Sort-Object Name) {
  $name = [Security.SecurityElement]::Escape($file.FullName)
  $entries += "<Component Id=`"Legal$i`"><File Source=`"$name`" KeyPath=`"yes`" /></Component>"
  $i++
}
$xml = '<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"><Fragment><DirectoryRef Id="INSTALLFOLDER"><Directory Id="LEGALFOLDER" Name="legal" /></DirectoryRef><ComponentGroup Id="LegalFiles" Directory="LEGALFOLDER">' + ($entries -join '') + '</ComponentGroup></Fragment></Wix>'
Set-Content -LiteralPath $LegalWxs -Value $xml -Encoding utf8


if ($Sign) { & (Join-Path $PSScriptRoot 'sign.ps1') (Join-Path $Stage 'shotclutch.exe') (Join-Path $Stage 'shotclutch-cli.exe') }

# ---- MSI ---------------------------------------------------------------------------------------
$Msi = Join-Path $Dist "shotclutch-$Version-windows-$Arch.msi"
Invoke-Native 'wix build' {
  & $Wix build (Join-Path $PSScriptRoot 'shotclutch.wxs') $LegalWxs -arch $Arch `
    -d "Version=$MsiVersion" -d "BinDir=$Stage" -d "IconPath=$(Join-Path $Root 'assets\app-icon\shotclutch.ico')" `
    -o $Msi
}
# wix writes its debug symbols (.wixpdb) next to the MSI; keep them out of the release assets.
Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($Msi, '.wixpdb'))
if ($Sign) { & (Join-Path $PSScriptRoot 'sign.ps1') $Msi }

# ---- portable zip ------------------------------------------------------------------------------
$Portable = Join-Path $TargetDir "windows-package\shotclutch-$Version-windows-$Arch-portable"
Assert-OwnedPath $Portable $TargetDir
Remove-Item -Recurse -Force -LiteralPath $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Stage '*.exe') $Portable
Copy-Item -LiteralPath $Legal -Destination $Portable -Recurse
# The marker that makes ShotClutch keep its data in .\data next to the exe instead of %APPDATA% (#139).
Set-Content -Encoding ascii -Path (Join-Path $Portable 'portable.txt') -Value @(
  'ShotClutch portable: preferences, presets, logs and crash recovery are kept in the data folder',
  'next to shotclutch.exe. Delete this file to use %APPDATA%\ShotClutch instead.'
)
foreach ($f in 'README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
# Licences of the embedded craft-fonts fonts (builds with CRAFT_FONTS_DIR) as OFL-<family>.txt.
if ($env:CRAFT_FONTS_DIR) {
  foreach ($lic in Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') -ErrorAction SilentlyContinue) {
    Copy-Item $lic.FullName (Join-Path $Portable "OFL-$($lic.Directory.Name).txt")
  }
}
$Zip = Join-Path $Dist "shotclutch-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

# Smoke-test the CLI when this machine can run it. An ARM64 build made on an x64 runner can't run
# here; .github/workflows/windows-arm64.yml installs and runs it on ARM64 instead.
$HostArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($Arch -ne 'arm64' -or $HostArch -eq 'arm64') {
  Invoke-Native 'shotclutch-cli --version' { & (Join-Path $Stage 'shotclutch-cli.exe') --version }
} else {
  Write-Output "skipping shotclutch-cli --version: an $Arch build doesn't run on this $HostArch machine"
}
Get-Item $Msi, $Zip | Format-Table Name, Length
