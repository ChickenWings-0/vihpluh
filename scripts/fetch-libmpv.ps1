<#
.SYNOPSIS
  Downloads the pinned libmpv development build for Windows x64 and prepares it for linking.

.DESCRIPTION
  Output: src-tauri/lib/windows-x64/
    libmpv-2.dll      runtime library (copied beside the executable by build.rs)
    libmpv.dll.a      MinGW import library (used directly by the GNU Rust toolchain)
    mpv.def           export list (generated from the DLL)
    mpv.lib           MSVC import library (generated when llvm-dlltool or lib.exe is available)
    include/mpv/*.h   headers

  Idempotent: a stamp file records the pinned tag + hash, and nothing is downloaded again when it matches.

.PARAMETER V3
  Fetch the x86-64-v3 (AVX2) build instead of the baseline x86-64 build.

.PARAMETER Force
  Re-download and re-extract even when the stamp matches.
#>
[CmdletBinding()]
param(
    [switch]$V3,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ── Pinned release (shinchiro/mpv-winbuild-cmake) ────────────────────────────
$Tag = '20261111'
$Commit = 'bcde6b93cc'
$Builds = @{
    'x86_64'    = @{ Sha256 = '273c079b11ea5575822f972756f4c9a71149a5c10130353abeb8586738938452' }
    'x86_64-v3' = @{ Sha256 = '19d9bb4c5f8249f48c2413a0b7c473116552b8cf55404a67a5a568b2d8b09bcb' }
}

$Arch = if ($V3) { 'x86_64-v3' } else { 'x86_64' }
$Asset = "mpv-dev-$Arch-$Tag-git-$Commit.7z"
$Sha256 = $Builds[$Arch].Sha256
$Url = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/$Tag/$Asset"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$LibRoot = Join-Path $RepoRoot 'src-tauri\lib'
$OutDir = Join-Path $LibRoot 'windows-x64'
$DownloadDir = Join-Path $LibRoot '.download'
$Stamp = Join-Path $OutDir '.stamp'
$StampValue = "$Asset $Sha256"

function Find-Tool([string[]]$Names, [string[]]$ExtraDirs = @()) {
    foreach ($name in $Names) {
        $cmd = Get-Command $name -ErrorAction SilentlyContinue
        if ($cmd) { return $cmd.Source }
        foreach ($dir in $ExtraDirs) {
            $candidate = Join-Path $dir $name
            if (Test-Path $candidate) { return $candidate }
        }
    }
    return $null
}

function Find-MsvcLib {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $null }
    $install = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $install) { return $null }
    $lib = Get-ChildItem -Path (Join-Path $install 'VC\Tools\MSVC') -Recurse -Filter lib.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'Hostx64\\x64' } | Select-Object -First 1
    if ($lib) { return $lib.FullName }
    return $null
}

# Reads the export name table of a PE32+ DLL without external tools.
function Get-DllExports([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $pe = [BitConverter]::ToInt32($bytes, 0x3C)
    $sectionCount = [BitConverter]::ToUInt16($bytes, $pe + 6)
    $optSize = [BitConverter]::ToUInt16($bytes, $pe + 20)
    $opt = $pe + 24
    $magic = [BitConverter]::ToUInt16($bytes, $opt)
    if ($magic -ne 0x20B) { throw "Not a PE32+ image: $Path" }
    $exportRva = [BitConverter]::ToUInt32($bytes, $opt + 112)
    $sections = $opt + $optSize
    $rvaToOffset = {
        param([uint32]$rva)
        for ($i = 0; $i -lt $sectionCount; $i++) {
            $s = $sections + 40 * $i
            $va = [BitConverter]::ToUInt32($bytes, $s + 12)
            $rawSize = [BitConverter]::ToUInt32($bytes, $s + 16)
            $raw = [BitConverter]::ToUInt32($bytes, $s + 20)
            if ($rva -ge $va -and $rva -lt ($va + $rawSize)) { return [int]($rva - $va + $raw) }
        }
        throw "RVA $rva is outside every section"
    }
    $dir = & $rvaToOffset $exportRva
    $nameCount = [BitConverter]::ToUInt32($bytes, $dir + 24)
    $namesOff = & $rvaToOffset ([BitConverter]::ToUInt32($bytes, $dir + 32))
    $names = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $nameCount; $i++) {
        $off = & $rvaToOffset ([BitConverter]::ToUInt32($bytes, $namesOff + 4 * $i))
        $end = $off
        while ($bytes[$end] -ne 0) { $end++ }
        $names.Add([System.Text.Encoding]::ASCII.GetString($bytes, $off, $end - $off))
    }
    return $names
}

if (-not $Force -and (Test-Path $Stamp) -and ((Get-Content $Stamp -Raw).Trim() -eq $StampValue) -and
    (Test-Path (Join-Path $OutDir 'libmpv-2.dll'))) {
    Write-Host "libmpv is up to date ($Asset)."
    exit 0
}

$SevenZip = Find-Tool @('7z.exe', '7za.exe', '7zr.exe') @(
    (Join-Path $env:ProgramFiles '7-Zip'),
    (Join-Path ${env:ProgramFiles(x86)} '7-Zip'))
if (-not $SevenZip) { throw '7-Zip was not found. Install it with: winget install --id 7zip.7zip' }

New-Item -ItemType Directory -Force -Path $DownloadDir | Out-Null
$Archive = Join-Path $DownloadDir $Asset

$needDownload = $true
if (Test-Path $Archive) {
    $have = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
    $needDownload = $have -ne $Sha256
}
if ($needDownload) {
    Write-Host "Downloading $Asset ..."
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $Url -OutFile $Archive -UseBasicParsing
}

$actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
if ($actual -ne $Sha256) {
    Remove-Item $Archive -Force
    throw "SHA-256 mismatch for $Asset`n  expected $Sha256`n  actual   $actual"
}
Write-Host 'SHA-256 verified.'

if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
& $SevenZip x $Archive "-o$OutDir" -y | Out-Null
if ($LASTEXITCODE -ne 0) { throw "7-Zip failed with exit code $LASTEXITCODE" }

$Dll = Join-Path $OutDir 'libmpv-2.dll'
if (-not (Test-Path $Dll)) { throw "libmpv-2.dll is missing from $Asset" }
if (-not (Test-Path (Join-Path $OutDir 'include\mpv\client.h'))) { throw "mpv headers are missing from $Asset" }

# Export list — always regenerated from the DLL so it cannot drift from the binary.
$Def = Join-Path $OutDir 'mpv.def'
$exports = Get-DllExports $Dll | Where-Object { $_ -like 'mpv_*' } | Sort-Object
$defLines = @('LIBRARY libmpv-2.dll', 'EXPORTS') + ($exports | ForEach-Object { "    $_" })
[System.IO.File]::WriteAllLines($Def, $defLines, (New-Object System.Text.ASCIIEncoding))
Write-Host "Wrote mpv.def ($($exports.Count) exports)."

# MSVC import library. The GNU toolchain links libmpv.dll.a directly and does not need it.
$MsvcLib = Join-Path $OutDir 'mpv.lib'
$dlltool = Find-Tool @('llvm-dlltool.exe') @((Join-Path $env:ProgramFiles 'LLVM\bin'))
if ($dlltool) {
    & $dlltool -m i386:x86-64 -D libmpv-2.dll -d $Def -l $MsvcLib
    if ($LASTEXITCODE -ne 0) { throw "llvm-dlltool failed with exit code $LASTEXITCODE" }
    Write-Host 'Generated mpv.lib with llvm-dlltool.'
}
else {
    $libExe = Find-MsvcLib
    if ($libExe) {
        & $libExe /nologo "/def:$Def" /name:libmpv-2.dll "/out:$MsvcLib" /machine:x64 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "lib.exe failed with exit code $LASTEXITCODE" }
        Remove-Item (Join-Path $OutDir 'mpv.exp') -ErrorAction SilentlyContinue
        Write-Host 'Generated mpv.lib with lib.exe.'
    }
    else {
        Write-Warning 'Neither llvm-dlltool nor MSVC lib.exe was found: mpv.lib was not generated.'
        Write-Warning 'The MSVC Rust toolchain needs it (winget install --id LLVM.LLVM, then re-run with -Force).'
        Write-Warning 'The GNU Rust toolchain links libmpv.dll.a and is unaffected.'
    }
}

Set-Content -Path $Stamp -Value $StampValue -Encoding ascii
Write-Host "libmpv ready in $OutDir"
