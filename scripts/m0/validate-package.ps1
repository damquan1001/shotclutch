param([string] $Dist = 'dist/release')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$zip = @(Get-ChildItem -LiteralPath $Dist -Filter '*-windows-x64-portable.zip')
$msi = @(Get-ChildItem -LiteralPath $Dist -Filter '*-windows-x64.msi')
if ($zip.Count -ne 1 -or $msi.Count -ne 1) { throw 'Expected one x64 ZIP and MSI' }
$scratch = Join-Path $root "target/m0/package-check-$([Guid]::NewGuid().ToString('N'))"
Expand-Archive -LiteralPath $zip[0].FullName -DestinationPath $scratch
$cli = Get-ChildItem -LiteralPath $scratch -Filter shotclutch-cli.exe -Recurse | Select-Object -First 1
if (-not $cli) { throw 'Packaged CLI missing' }
$bin = $cli.Directory.FullName
foreach ($name in 'shotclutch.exe','portable.txt','legal/NOTICE','legal/build-info.json','legal/ATTRIBUTION.md','legal/LICENSE-MIT','legal/LICENSE-APACHE','legal/OFL-Inter.txt','legal/OFL-NotoSerif.txt','legal/OFL-JetBrainsMono.txt') {
  if (-not (Test-Path -LiteralPath (Join-Path $bin $name))) { throw "Package missing $name" }
}
$version = & $cli.FullName --version
if ($LASTEXITCODE -ne 0 -or $version -notlike 'shotclutch-cli *') { throw 'Packaged CLI failed version smoke' }
& $cli.FullName --demo inspect project | Set-Content -LiteralPath (Join-Path $scratch 'project.json')
if ($LASTEXITCODE -ne 0) { throw 'Packaged demo inspection failed' }
$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.OpenDatabase($msi[0].FullName, 0)
$view = $database.OpenView('SELECT `Property`, `Value` FROM `Property`')
$view.Execute()
$properties = @{}
while ($record = $view.Fetch()) { $properties[$record.StringData(1)] = $record.StringData(2) }
if ($properties.ProductName -ne 'ShotClutch' -or $properties.UpgradeCode -ne '{BB349D14-E803-4D51-B94A-17D55882EE78}') { throw 'MSI identity invalid' }
$dialogView = $database.OpenView('SELECT `Dialog` FROM `Dialog`')
$dialogView.Execute()
$dialogs = @()
while ($record = $dialogView.Fetch()) { $dialogs += $record.StringData(1) }
foreach ($dialog in 'WelcomeDlg','LicenseAgreementDlg','InstallDirDlg','ProgressDlg','ExitDialog','MaintenanceWelcomeDlg') {
  if ($dialog -notin $dialogs) { throw "MSI setup wizard missing $dialog" }
}
$launchView = $database.OpenView("SELECT ``Condition`` FROM ``ControlEvent`` WHERE ``Dialog_`` = 'ExitDialog' AND ``Control_`` = 'Finish' AND ``Event`` = 'DoAction' AND ``Argument`` = 'LaunchShotclutch'")
$launchView.Execute()
$launch = $launchView.Fetch()
if (-not $launch -or $launch.StringData(1) -notmatch 'NOT Installed' -or $launch.StringData(1) -notmatch 'WIXUI_EXITDIALOGOPTIONALCHECKBOX') { throw 'MSI finish launch must be checkbox-gated and initial-install only' }
$actionView = $database.OpenView("SELECT ``Type``, ``Source`` FROM ``CustomAction`` WHERE ``Action`` = 'LaunchShotclutch'")
$actionView.Execute()
$action = $actionView.Fetch()
if (-not $action -or $action.StringData(2) -ne 'ShotclutchExe' -or ($action.IntegerData(1) -band 63) -ne 18) { throw 'MSI launch action must target the installed application' }
foreach ($sequence in 'InstallExecuteSequence','InstallUISequence') {
  $sequenceView = $database.OpenView("SELECT ``Action`` FROM ``$sequence`` WHERE ``Action`` = 'LaunchShotclutch'")
  $sequenceView.Execute()
  if ($sequenceView.Fetch()) { throw 'MSI must not auto-launch during a quiet install' }
}
if ($properties.WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT -ne 'Launch ShotClutch') { throw 'MSI launch checkbox missing' }
$registryView = $database.OpenView('SELECT `Key`, `Value` FROM `Registry`')
$registryView.Execute()
$nativeAssociation = $false
$nativeOpenCommand = $false
while ($row = $registryView.Fetch()) {
  $key = $row.StringData(1)
  if ($key -eq 'Software\Classes\.scproj' -and $row.StringData(2) -eq 'ShotClutch.Project') { $nativeAssociation = $true }
  if ($key -eq 'Software\Classes\ShotClutch.Project\shell\open\command' -and $row.StringData(2) -eq '"[#ShotclutchExe]" "%1"') { $nativeOpenCommand = $true }
  if ($key -eq 'Software\Classes\.fcproj') { throw 'MSI must not claim the legacy project association' }
}
if (-not $nativeAssociation -or -not $nativeOpenCommand) { throw 'MSI .scproj association/open command missing' }
$fileView = $database.OpenView('SELECT `FileName` FROM `File`')
$fileView.Execute()
$names = @()
while ($record = $fileView.Fetch()) { $names += $record.StringData(1) }
foreach ($name in 'NOTICE','ATTRIBUTION.md','LICENSE-MIT','LICENSE-APACHE') {
  if (-not ($names | Where-Object { ($_ -split '\|')[-1] -eq $name })) { throw "MSI missing $name" }
}
@{result='passed'; version=$version; zip_sha256=(Get-FileHash $zip[0].FullName).Hash; msi_sha256=(Get-FileHash $msi[0].FullName).Hash; installer_wizard='present'; finish_launch='checkbox-gated; absent from silent install sequences'; clean_machine_install='not performed'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $scratch 'validation.json')
Write-Output "Package checks passed; evidence: $scratch (clean-machine installation is a separate gate)"
