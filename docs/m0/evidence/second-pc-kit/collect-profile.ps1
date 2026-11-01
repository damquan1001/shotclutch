param([string] $Label = 'second-pc')
$ErrorActionPreference = 'Stop'
$os = Get-CimInstance Win32_OperatingSystem
$cpu = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
$system = Get-CimInstance Win32_ComputerSystem
$gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,DriverDate,VideoProcessor,CurrentHorizontalResolution,CurrentVerticalResolution)
$hashes = @(Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'packages') -File | ForEach-Object { @{name=$_.Name;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash} })
$report = @{label=$Label;captured_utc=[DateTime]::UtcNow.ToString('o');os=@{caption=$os.Caption;version=$os.Version;build=$os.BuildNumber;architecture=$os.OSArchitecture};cpu=$cpu;physical_memory_bytes=$system.TotalPhysicalMemory;gpu=$gpu;packages=$hashes;fixture_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'fixture-1080p.mp4')).Hash;serial_numbers_collected=$false;credentials_collected=$false;measurements=@{physical_av_drift='unavailable';peak_working_set='not measured';dropped_frames='not measured'};result='profile_only_not_a_workflow_pass'}
$folder = Join-Path $PSScriptRoot 'results'
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$file = Join-Path $folder ('profile-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ') + '.json')
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $file -Encoding UTF8
Write-Output "Profile captured: $file"
