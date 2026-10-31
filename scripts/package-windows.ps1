<#
.SYNOPSIS
  Build QPE's portable x64 Windows exe locally.

.DESCRIPTION
  Mirrors the `windows` job in .github/workflows/ci.yml:
    * fetches a shinchiro libmpv-2.dll and a static BtbN ffmpeg.exe
    * creates the MSVC import library mpv.lib from the DLL's exports
    * runs the tests and `cargo build --release`
    * copies the result to dist/QPE-<version>-windows-x64.exe

  build.rs embeds libmpv-2.dll and ffmpeg.exe into qpe.exe. This keeps the
  existing single-file behavior: src/bundle.rs unpacks them to
  %LOCALAPPDATA%\QPE\<build-id> on first run.

  Run this from an "x64 Native Tools Command Prompt for VS 2022" (or any shell
  where the MSVC developer environment, including link.exe, is active).
  7-Zip is required unless -MpvDir is supplied.

.PARAMETER Version
  Name suffix for the packaged exe. Defaults to "dev".

.PARAMETER MpvDir
  Directory containing libmpv-2.dll. If omitted, the latest shinchiro
  mpv-dev-x86_64 build is downloaded and cached under target\windows-deps.

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
  .\scripts\package-windows.ps1 -Version v0.5.0

.EXAMPLE
  .\scripts\package-windows.ps1 -MpvDir C:\deps\mpv -FfmpegDir C:\deps\ffmpeg\bin -SkipTests
#>
[CmdletBinding()]
param(
    [string]$Version = 'dev',
    [string]$MpvDir,
    [string]$FfmpegDir,
    [string]$DepsDir,
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message" -ForegroundColor Green
}

function Stop-Build {
    param([string]$Message)
    throw $Message
}

function Require-Command {
    param([string]$Name, [string]$Hint)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Stop-Build "$Name was not found. $Hint"
    }
}

function Find-SevenZip {
    $cmd = Get-Command 7z.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }

    $roots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) | Where-Object { $_ }
    foreach ($base in $roots) {
        $candidate = Join-Path $base '7-Zip\7z.exe'
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
    return $null
}

function Find-DepDir {
    param([string]$Filter)
    if (-not (Test-Path -LiteralPath $DepsDir)) { return $null }
    $file = Get-ChildItem -LiteralPath $DepsDir -Filter $Filter -Recurse -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($file) { return $file.DirectoryName }
    return $null
}

function Get-GitHubHeaders {
    $headers = @{
        'User-Agent' = 'qpe-windows-build'
        'Accept'     = 'application/vnd.github+json'
    }
    $token = $env:GITHUB_TOKEN
    if (-not $token) { $token = $env:GH_TOKEN }
    if ($token) {
        $headers['Authorization'] = "Bearer $token"
    }
    return $headers
}

function Download-File {
    param([string]$Uri, [string]$Destination)
    $tmp = "$Destination.part"
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force }
    $null = Invoke-WebRequest -Uri $Uri -OutFile $tmp -UseBasicParsing
    Move-Item -LiteralPath $tmp -Destination $Destination -Force
}

function Install-MpvDev {
    $archive = Join-Path $DepsDir 'mpv-dev.7z'
    $extract = Join-Path $DepsDir 'mpv'

    if (-not (Test-Path -LiteralPath $archive)) {
        Write-Step 'Looking up the latest shinchiro mpv Windows build...'
        $headers = Get-GitHubHeaders
        try {
            $release = Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest'
        } catch {
            Stop-Build "Can't query the shinchiro release: $($_.Exception.Message). Set GITHUB_TOKEN or GH_TOKEN if this is a rate limit."
        }
        $asset = @($release.assets) |
            Where-Object { $_.name -like 'mpv-dev-x86_64-2*.7z' } |
            Select-Object -First 1
        if (-not $asset) {
            Stop-Build 'The latest shinchiro release has no mpv-dev-x86_64-2*.7z asset.'
        }
        Write-Step "Downloading $($asset.name)"
        Download-File -Uri $asset.browser_download_url -Destination $archive
    }

    if (Test-Path -LiteralPath $extract) {
        Remove-Item -LiteralPath $extract -Recurse -Force
    }
    $null = New-Item -ItemType Directory -Force -Path $extract

    $seven = Find-SevenZip
    if (-not $seven) {
        Stop-Build '7-Zip was not found. Install it or pass -MpvDir with libmpv-2.dll.'
    }
    & $seven x $archive "-o$extract" -y | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Stop-Build "7-Zip failed to extract $archive."
    }

    $dll = Get-ChildItem -LiteralPath $extract -Filter 'libmpv-2.dll' -Recurse -File |
        Select-Object -First 1
    if (-not $dll) {
        Stop-Build "libmpv-2.dll is missing from $archive."
    }
    return $dll.DirectoryName
}

function Install-Ffmpeg {
    $archive = Join-Path $DepsDir 'ffmpeg.zip'
    $extract = Join-Path $DepsDir 'ffmpeg'

    if (-not (Test-Path -LiteralPath $archive)) {
        $url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'
        Write-Step 'Downloading static ffmpeg...'
        Download-File -Uri $url -Destination $archive
    }

    if (Test-Path -LiteralPath $extract) {
        Remove-Item -LiteralPath $extract -Recurse -Force
    }
    $null = New-Item -ItemType Directory -Force -Path $extract
    Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force

    $exe = Get-ChildItem -LiteralPath $extract -Filter 'ffmpeg.exe' -Recurse -File |
        Select-Object -First 1
    if (-not $exe) {
        Stop-Build "ffmpeg.exe is missing from $archive."
    }
    return $exe.DirectoryName
}

function Ensure-MpvLib {
    param([string]$Dir)
    $dll = Join-Path $Dir 'libmpv-2.dll'
    if (-not (Test-Path -LiteralPath $dll)) {
        Stop-Build "libmpv-2.dll was not found in $Dir."
    }

    $lib = Join-Path $Dir 'mpv.lib'
    if (Test-Path -LiteralPath $lib) { return }

    Require-Command dumpbin.exe 'Run from an x64 Native Tools Command Prompt for VS 2022.'
    Require-Command lib.exe 'Run from an x64 Native Tools Command Prompt for VS 2022.'
    Write-Step 'Creating mpv.lib from libmpv-2.dll exports...'

    # Use relative names so paths with spaces (a checkout under "My Projects") still work.
    Push-Location -LiteralPath $Dir
    try {
        $exports = & dumpbin.exe /exports 'libmpv-2.dll' | ForEach-Object {
            if ($_ -match '\s(mpv_\w+)$') { $Matches[1] }
        }
        if (-not $exports) {
            Stop-Build "Couldn't read any mpv_* exports from $dll."
        }

        Set-Content -LiteralPath 'mpv.def' -Value (@('EXPORTS') + @($exports)) -Encoding ASCII
        & lib.exe /nologo '/def:mpv.def' /name:libmpv-2.dll '/out:mpv.lib' /machine:x64 | Out-Null
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath 'mpv.lib')) {
            Stop-Build "lib.exe could not create $lib."
        }
    } finally {
        Pop-Location
    }
}

$root = Split-Path -Parent $PSScriptRoot
$startDir = (Get-Location).Path
Push-Location -LiteralPath $root
try {
    if ($env:OS -ne 'Windows_NT') {
        Stop-Build 'This script builds the Windows package and must run on Windows.'
    }
    if ($Version -match '[\\/:*?"<>|]') {
        Stop-Build "Version '$Version' contains characters that cannot be used in a file name."
    }

    Require-Command cargo.exe 'Install Rust with rustup first.'
    Require-Command link.exe 'Run from an x64 Native Tools Command Prompt for VS 2022.'
    if (-not (Get-Command rc.exe -ErrorAction SilentlyContinue)) {
        Write-Warning 'rc.exe was not found; qpe.exe will be built without an icon.'
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
        Stop-Build 'build.rs did not embed libmpv/ffmpeg. Run "cargo clean -p qpe" and try again.'
    }

    $exe = Join-Path $root 'target\release\qpe.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        Stop-Build "Build finished but $exe does not exist."
    }
    $size = (Get-Item -LiteralPath $exe).Length
    if ($size -lt 30MB) {
        $mb = [math]::Round($size / 1MB, 1)
        Stop-Build "qpe.exe is only $mb MB, so the embedded tools are probably missing. Run `"cargo clean -p qpe`" and try again."
    }

    $dist = Join-Path $root 'dist'
    $null = New-Item -ItemType Directory -Force -Path $dist
    $out = Join-Path $dist "QPE-$Version-windows-x64.exe"
    Copy-Item -LiteralPath $exe -Destination $out -Force

    Write-Host ''
    Write-Host "Built $out" -ForegroundColor Green
    Write-Host 'The exe is self-contained; first run still extracts libmpv and ffmpeg to %LOCALAPPDATA%\QPE\<build-id>.'
} finally {
    Pop-Location
}
