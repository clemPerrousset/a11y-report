# Installs the latest a11y-report release on Windows.
#   irm https://raw.githubusercontent.com/clemPerrousset/a11y-report/main/install.ps1 | iex
# Options (environment variables):
#   $env:A11Y_REPORT_VERSION = "v0.1.0"   install a specific version (default: latest)
#   $env:A11Y_REPORT_INSTALL_DIR = "..."  where to put the binary
$ErrorActionPreference = "Stop"

$Repo = "clemPerrousset/a11y-report"
$Bin = "a11y-report"
$Target = "x86_64-pc-windows-msvc"
$Version = if ($env:A11Y_REPORT_VERSION) { $env:A11Y_REPORT_VERSION } else { "latest" }
$InstallDir = if ($env:A11Y_REPORT_INSTALL_DIR) { $env:A11Y_REPORT_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "a11y-report\bin" }

$Asset = "$Bin-$Target.zip"
$Url = if ($Version -eq "latest") {
    "https://github.com/$Repo/releases/latest/download/$Asset"
} else {
    "https://github.com/$Repo/releases/download/$Version/$Asset"
}

$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("a11y-report-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile (Join-Path $Tmp $Asset) -UseBasicParsing
    Expand-Archive -Path (Join-Path $Tmp $Asset) -DestinationPath $Tmp -Force
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $Tmp "$Bin-$Target\$Bin.exe") (Join-Path $InstallDir "$Bin.exe") -Force
    Write-Host "Installed $(Join-Path $InstallDir "$Bin.exe")"

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not ($UserPath -split ";" | Where-Object { $_ -eq $InstallDir })) {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        Write-Host "Added $InstallDir to your user PATH. Open a new terminal to use a11y-report."
    }
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
