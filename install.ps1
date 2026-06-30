# ClawHive OS — Windows installer (PowerShell)
# Usage:
#   irm https://raw.githubusercontent.com/crediblemark-official/clawhive/master/install.ps1 | iex
#
# To use your own domain, replace the raw GitHub URL above and host this script there.

$ErrorActionPreference = "Stop"

$Repo = "crediblemark-official/clawhive"
$Binary = "clawhive.exe"
$InstallDir = if ($env:CLAWHIVE_INSTALL_DIR) { $env:CLAWHIVE_INSTALL_DIR } else { "$env:LOCALAPPDATA\ClawHive\bin" }
$CargoBuild = if ($env:CLAWHIVE_CARGO_BUILD) { $env:CLAWHIVE_CARGO_BUILD } else { "0" }

$Platform = "windows"
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }

# Detect ARM64 Windows
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_ARCHITEW6432 -eq "ARM64") {
    $Arch = "aarch64"
}

Write-Host "Installing ClawHive OS for $Platform-$Arch..."

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$Downloaded = $false
if ($CargoBuild -eq "0") {
    $LatestUrl = "https://api.github.com/repos/$Repo/releases/latest"
    $AssetName = "clawhive-$Platform-$Arch.zip"

    try {
        $Release = Invoke-RestMethod -Uri $LatestUrl -UseBasicParsing -TimeoutSec 15
        $Asset = $Release.assets | Where-Object { $_.name -eq $AssetName } | Select-Object -First 1

        if ($Asset) {
            $TmpDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
            New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
            $ZipPath = Join-Path $TmpDir $AssetName

            Write-Host "Downloading $AssetName..."
            Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $ZipPath -UseBasicParsing
            Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force
            Copy-Item -Path (Join-Path $TmpDir $Binary) -Destination (Join-Path $InstallDir $Binary) -Force
            Remove-Item -Recurse -Force $TmpDir
            $Downloaded = $true
        }
    } catch {
        Write-Host "No prebuilt binary found or download failed. Falling back to cargo build..."
    }
}

if (-not $Downloaded) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "Rust/Cargo is required but not installed. Install Rust first:"
        Write-Host '  https://rustup.rs/'
        exit 1
    }

    $TmpDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
    git clone --depth 1 "https://github.com/$Repo.git" "$TmpDir\clawhive" 2>$null
    if (-not (Test-Path "$TmpDir\clawhive")) {
        Write-Host "Failed to clone repository. Make sure git is installed."
        exit 1
    }
    cargo build --release --manifest-path "$TmpDir\clawhive\Cargo.toml"
    Copy-Item -Path "$TmpDir\clawhive\target\release\$Binary" -Destination (Join-Path $InstallDir $Binary) -Force
    Remove-Item -Recurse -Force $TmpDir
}

# Add to PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "Added $InstallDir to user PATH."
}

Write-Host ""
Write-Host "ClawHive OS installed to: $InstallDir\$Binary"
Write-Host "Run 'clawhive --help' to get started."
Write-Host "Run 'clawhive setup' for initial configuration wizard."
