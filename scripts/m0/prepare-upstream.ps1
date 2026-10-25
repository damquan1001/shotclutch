param([string] $Revision = '3a5f217b348bf0c9dc1678f305c34b6c88617f37')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if ($Revision -notmatch '^[a-fA-F0-9]{40}$') { throw 'Provide the complete immutable commit hash' }
$destination = Join-Path $root 'target/m0/upstream'
if (Test-Path -LiteralPath $destination) { throw "Baseline already exists: $destination; preserve it or choose a fresh workspace" }
New-Item -ItemType Directory -Force -Path (Join-Path $root 'target/m0') | Out-Null
$archive = Join-Path $root 'target/m0/upstream.zip'
& git -C $root archive --format=zip "--output=$archive" $Revision
if ($LASTEXITCODE -ne 0) { throw 'Could not archive inherited revision' }
Expand-Archive -LiteralPath $archive -DestinationPath $destination
@{revision=$Revision; archive_sha256=(Get-FileHash -LiteralPath $archive).Hash; destination=$destination; repairs=@()} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'target/m0/upstream-manifest.json')
Write-Output "Untouched baseline prepared: $destination"
