param([Parameter(Mandatory)][string]$Executable,[Parameter(Mandatory)][string]$Destination,[switch]$CpuOnly)
$ErrorActionPreference = "Stop"
$source = Get-Item -LiteralPath $Executable
if (Test-Path -LiteralPath $Destination) { throw "Choose a new preview directory." }
$stage = New-Item -ItemType Directory -Path $Destination
$flavor = if ($CpuOnly) { "cpu" } else { "vulkan" }
$expected = if ($CpuOnly) { "7c3ec4ff5cdddc272cb1d6b52a05c8751dc566c7105750c10ed3688bbdbf8fa8" } else { "7859e96ba91201a41e2769d4456fe86e8968ba93c6794eeb4fe49b7537a17628" }
$zip = Join-Path $stage.FullName "llama-b12483.zip"
Invoke-WebRequest -Uri "https://github.com/ggml-org/llama.cpp/releases/download/b12483/llama-b12483-bin-win-$flavor-x64.zip" -OutFile $zip
if ((Get-FileHash -LiteralPath $zip).Hash -ne $expected) { throw "Unexpected llama.cpp runtime checksum." }
Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $stage.FullName "llama")
Copy-Item -LiteralPath $source.FullName -Destination (Join-Path $stage.FullName "shotclutch.exe")
Set-Content -LiteralPath (Join-Path $stage.FullName "portable.txt") -Value "Experimental preview data stays beside this executable."
Set-Content -LiteralPath (Join-Path $stage.FullName "README.txt") -Value @"
Experimental Smart Cut preview. Open shotclutch.exe, then open a project copy.
In Smart Cut, expand Evidence-based take review (experimental), choose a model,
install it, and click Review takes with evidence. Model downloads are local and
checksum verified. Keep the llama and data folders beside the executable.
Undo returns to the V6 checkpoint. Undo again returns to the preceding edit.
This workflow is opt-in; release promotion requires the recorded accuracy gates.
"@
@{version="b12483";flavor=$flavor;sha256=$expected;source="https://github.com/ggml-org/llama.cpp/releases/tag/b12483"} |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage.FullName "runtime.json")
