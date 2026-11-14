<#
.SYNOPSIS
  Packs the Windows release build into a portable zip.

.DESCRIPTION
  Run after `pnpm tauri build`. The zip holds vihpluh.exe, libmpv-2.dll (and WebView2Loader.dll
  when the build has one), the licence files,
  and an empty file named `portable`: with that marker next to the executable, vihpluh keeps
  its settings, library, and cache in a `data` folder beside itself instead of in the user
  profile (ARCHITECTURE §9.1), so the folder can live on a USB stick.

  Output: src-tauri/target/release/bundle/portable/vihpluh-<version>-windows-x64-portable.zip
  and a .sha256 file next to it.

.PARAMETER TargetDir
  The cargo profile directory that holds vihpluh.exe (default: src-tauri/target/release).
#>
[CmdletBinding()]
param(
    [string]$TargetDir = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $TargetDir) { $TargetDir = Join-Path $RepoRoot 'src-tauri\target\release' }

$Exe = Join-Path $TargetDir 'vihpluh.exe'
$Dll = Join-Path $TargetDir 'libmpv-2.dll'
foreach ($file in @($Exe, $Dll)) {
    if (-not (Test-Path $file)) { throw "$file is missing. Run 'pnpm tauri build' first." }
}

$Version = (Get-Content (Join-Path $RepoRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$Name = "vihpluh-$Version-windows-x64-portable"
$OutDir = Join-Path $TargetDir 'bundle\portable'
$Stage = Join-Path $OutDir $Name
$Zip = Join-Path $OutDir "$Name.zip"

if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Exe, $Dll -Destination $Stage
# A build made with the GNU toolchain loads WebView2 through this DLL (cargo puts it next to
# the executable); an MSVC build links the loader in and has no such file.
$Loader = Join-Path $TargetDir 'WebView2Loader.dll'
if (Test-Path $Loader) { Copy-Item $Loader -Destination $Stage }
Copy-Item (Join-Path $RepoRoot 'LICENSE') -Destination $Stage
Copy-Item (Join-Path $RepoRoot 'THIRD_PARTY_NOTICES.md') -Destination $Stage
# The marker: its presence is what matters, the file stays empty.
New-Item -ItemType File -Path (Join-Path $Stage 'portable') | Out-Null

if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal
Remove-Item -Recurse -Force $Stage

$Hash = (Get-FileHash -Algorithm SHA256 $Zip).Hash.ToLower()
"$Hash  $Name.zip" | Set-Content -Encoding ascii -NoNewline "$Zip.sha256"
Write-Host "$Zip"
Write-Host "sha256 $Hash"
