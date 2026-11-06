# Small helpers for package-windows.ps1, which dot-sources this file.

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
        'User-Agent' = 'glint-windows-build'
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

function Clear-Dir {
    param([string]$Path)
    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
    $null = New-Item -ItemType Directory -Force -Path $Path
}
