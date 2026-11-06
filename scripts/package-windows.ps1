<#
.SYNOPSIS
  Build Chyk's portable x64 Windows exe locally.

.DESCRIPTION
  Mirrors the `windows` job in .github/workflows/ci.yml:
    * fetches a shinchiro libmpv-2.dll and a static BtbN ffmpeg.exe
    * creates the MSVC import library mpv.lib from the DLL's exports
    * runs the tests and `cargo build --release`
    * copies the result to dist/Chyk-windows-x64.exe

  build.rs embeds libmpv-2.dll and ffmpeg.exe into chyk.exe. This keeps the
  existing single-file behavior: src/bundle.rs unpacks them to
  %LOCALAPPDATA%\Chyk\<build-id> on first run.

  Run this from an "x64 Native Tools Command Prompt for VS 2022" (or any shell
  where the MSVC developer environment, including link.exe, is active).
  The mpv archive is .7z. Extraction tries Windows' built-in tar.exe first,
  then an installed 7-Zip, then a downloaded standalone 7zr.exe, so no 7-Zip
  install is required.

.PARAMETER MpvDir
  Directory containing libmpv-2.dll. If omitted, the latest shinchiro
  mpv-dev-x86_64 build is downloaded and cached under target\windows-deps.
  Supplying this skips the download and the 7z extraction entirely.

.PARAMETER FfmpegDir
  Directory containing ffmpeg.exe (and ffprobe.exe, unless -SkipTests).
  If omitted, a static BtbN build is downloaded and cached.

.PARAMETER DepsDir
  Download/cache directory. Defaults to target\windows-deps.

.PARAMETER SkipTests
  Skip `cargo test`. Useful when ffprobe.exe is not available.

.EXAMPLE
  .\scripts\package-windows.ps1

.EXAMPLE
  .\scripts\package-windows.ps1 -MpvDir C:\deps\mpv -FfmpegDir C:\deps\ffmpeg\bin -SkipTests
#>
[CmdletBinding()]
param(
    [string]$MpvDir,
    [string]$FfmpegDir,
    [string]$DepsDir,
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

. (Join-Path $PSScriptRoot 'windows\util.ps1')
. (Join-Path $PSScriptRoot 'windows\deps.ps1')

$root = Split-Path -Parent $PSScriptRoot
$startDir = (Get-Location).Path
Push-Location -LiteralPath $root
try {
    if ($env:OS -ne 'Windows_NT') {
        Stop-Build 'This script builds the Windows package and must run on Windows.'
    }

    Require-Command cargo.exe 'Install Rust with rustup first.'
    Require-Command link.exe 'Run from an x64 Native Tools Command Prompt for VS 2022.'
    if (-not (Get-Command rc.exe -ErrorAction SilentlyContinue)) {
        Write-Warning 'rc.exe was not found; chyk.exe will be built without an icon.'
    }

    if (-not $DepsDir) {
        $DepsDir = Join-Path $root 'target\windows-deps'
    } elseif (-not [System.IO.Path]::IsPathRooted($DepsDir)) {
        $DepsDir = Join-Path $startDir $DepsDir
    }
    $null = New-Item -ItemType Directory -Force -Path $DepsDir

    # libmpv: use the supplied directory or the cached/downloaded dev build.
    if ($MpvDir) {
        if (-not [System.IO.Path]::IsPathRooted($MpvDir)) {
            $MpvDir = Join-Path $startDir $MpvDir
        }
        if (-not (Test-Path -LiteralPath $MpvDir)) {
            Stop-Build "MpvDir does not exist: $MpvDir"
        }
        $MpvDir = (Resolve-Path -LiteralPath $MpvDir).Path
    } else {
        $MpvDir = Find-DepDir 'libmpv-2.dll'
        if (-not $MpvDir) {
            $MpvDir = Install-MpvDev
        }
    }
    Ensure-MpvLib -Dir $MpvDir

    # ffmpeg: use the supplied directory or the cached/downloaded static build.
    if ($FfmpegDir) {
        if (-not [System.IO.Path]::IsPathRooted($FfmpegDir)) {
            $FfmpegDir = Join-Path $startDir $FfmpegDir
        }
        if (-not (Test-Path -LiteralPath $FfmpegDir)) {
            Stop-Build "FfmpegDir does not exist: $FfmpegDir"
        }
        $FfmpegDir = (Resolve-Path -LiteralPath $FfmpegDir).Path
    } else {
        $FfmpegDir = Find-DepDir 'ffmpeg.exe'
        if (-not $FfmpegDir) {
            $FfmpegDir = Install-Ffmpeg
        }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $FfmpegDir 'ffmpeg.exe'))) {
        Stop-Build "ffmpeg.exe was not found in $FfmpegDir."
    }

    # build.rs embeds exactly these two files, no neighbours. Warn if a custom build
    # looks like it needs sibling DLLs at runtime.
    $mpvNeighbours = Get-ChildItem -LiteralPath $MpvDir -Filter '*.dll' -File |
        Where-Object { $_.Name -ne 'libmpv-2.dll' }
    if ($mpvNeighbours) {
        Write-Warning "libmpv-2.dll has other DLLs next to it; only libmpv-2.dll is embedded. Use the shinchiro mpv-dev build or bundle the extras yourself."
    }
    $ffmpegNeighbours = Get-ChildItem -LiteralPath $FfmpegDir -Filter '*.dll' -File
    if ($ffmpegNeighbours) {
        Write-Warning 'ffmpeg.exe has DLLs next to it; only ffmpeg.exe is embedded. Use a static ffmpeg build (not the -shared variant).'
    }

    $env:MPV_LIB_DIR = $MpvDir
    $env:PATH = "$FfmpegDir$([System.IO.Path]::PathSeparator)$env:PATH"

    if (-not $SkipTests -and -not (Get-Command ffprobe.exe -ErrorAction SilentlyContinue)) {
        Stop-Build 'ffprobe.exe was not found. Use -SkipTests or add ffprobe.exe to FfmpegDir/PATH.'
    }

    Write-Step "libmpv: $MpvDir"
    Write-Step "ffmpeg: $FfmpegDir"

    if (-not $SkipTests) {
        Write-Step 'Running tests...'
        & cargo test
        if ($LASTEXITCODE -ne 0) {
            Stop-Build 'cargo test failed.'
        }
    }

    Write-Step 'Building release exe...'
    $buildLogPath = Join-Path $DepsDir 'cargo-build.log'
    & cargo build --release 2>&1 | Tee-Object -FilePath $buildLogPath | Out-Host
    if ($LASTEXITCODE -ne 0) {
        Stop-Build 'cargo build --release failed.'
    }
    $buildLog = Get-Content -LiteralPath $buildLogPath -Raw
    if ($buildLog -match 'will need them next to it') {
        Stop-Build 'build.rs did not embed libmpv/ffmpeg. Run "cargo clean -p chyk" and try again.'
    }

    $exe = Join-Path $root 'target\release\chyk.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        Stop-Build "Build finished but $exe does not exist."
    }
    $size = (Get-Item -LiteralPath $exe).Length
    if ($size -lt 30MB) {
        $mb = [math]::Round($size / 1MB, 1)
        Stop-Build "chyk.exe is only $mb MB, so the embedded tools are probably missing. Run `"cargo clean -p chyk`" and try again."
    }

    $dist = Join-Path $root 'dist'
    $null = New-Item -ItemType Directory -Force -Path $dist
    $out = Join-Path $dist "Chyk-windows-x64.exe"
    Copy-Item -LiteralPath $exe -Destination $out -Force

    Write-Host ''
    Write-Host "Built $out" -ForegroundColor Green
    Write-Host 'The exe is self-contained; first run still extracts libmpv and ffmpeg to %LOCALAPPDATA%\Chyk\<build-id>.'
} finally {
    Pop-Location
}
