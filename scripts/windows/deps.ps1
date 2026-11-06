# libmpv and ffmpeg for package-windows.ps1, which dot-sources this file.
# Uses the script's $DepsDir as the download cache.

function Expand-MpvArchive {
    param([string]$Archive, [string]$Destination)

    # 1. Windows 10/11 ships bsdtar, whose libarchive can read 7z.
    $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
    if ($tar) {
        Write-Step 'Extracting libmpv with Windows tar.exe...'
        Clear-Dir -Path $Destination
        # Relative archive name avoids tar mistaking the drive letter for a remote host.
        $archiveDir = Split-Path -Parent $Archive
        $archiveName = Split-Path -Leaf $Archive
        $tarExit = 1
        Push-Location -LiteralPath $archiveDir
        try {
            & $tar.Source -xf $archiveName -C $Destination 2>&1 | Out-Null
            $tarExit = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        $dll = Get-ChildItem -LiteralPath $Destination -Filter 'libmpv-2.dll' -Recurse -File -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($tarExit -eq 0 -and $dll) { return $dll.DirectoryName }
        Write-Warning "Windows tar.exe could not extract $Archive; trying 7-Zip."
    }

    # 2. A normal 7-Zip installation, if present.
    $seven = Find-SevenZip
    if ($seven) {
        Write-Step 'Extracting libmpv with 7-Zip...'
        Clear-Dir -Path $Destination
        & $seven x $Archive "-o$Destination" -y | Out-Null
        $dll = Get-ChildItem -LiteralPath $Destination -Filter 'libmpv-2.dll' -Recurse -File -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($LASTEXITCODE -eq 0 -and $dll) { return $dll.DirectoryName }
        Write-Warning "7-Zip could not extract $Archive; trying 7zr.exe."
    }

    # 3. Last resort: the small standalone 7zr.exe from 7-zip.org, no install needed.
    $sevenR = Join-Path $DepsDir '7zr.exe'
    if (-not (Test-Path -LiteralPath $sevenR)) {
        Write-Step 'Downloading standalone 7zr.exe...'
        Download-File -Uri 'https://www.7-zip.org/a/7zr.exe' -Destination $sevenR
    }
    Write-Step 'Extracting libmpv with 7zr.exe...'
    Clear-Dir -Path $Destination
    & $sevenR x $Archive "-o$Destination" -y | Out-Null
    $dll = Get-ChildItem -LiteralPath $Destination -Filter 'libmpv-2.dll' -Recurse -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($LASTEXITCODE -eq 0 -and $dll) { return $dll.DirectoryName }

    Stop-Build "Could not extract $Archive. Install 7-Zip, or pass -MpvDir with libmpv-2.dll."
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

    return Expand-MpvArchive -Archive $archive -Destination $extract
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
