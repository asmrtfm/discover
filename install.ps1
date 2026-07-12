# install.ps1 — idempotent Windows installer for discover + static ast-grep binaries.
#
# Downloads the latest GitHub Release (no local build, no Rust toolchain) and
# installs to user scope. Safe to re-run: exits without changes when the
# installed version already matches the latest release.
#
#   irm https://github.com/asmrtfm/discover/releases/latest/download/install.ps1 | iex
#
# Installs ast-grep.exe / sg.exe (and jq.exe if absent) plus the discover
# framework tree. The discover CLI itself is bash — run it from Git Bash or
# WSL; the binaries work everywhere.
#
# Environment overrides:
#   DISCOVER_REPO         GitHub repo to install from (default: asmrtfm/discover)
#   DISCOVER_INSTALL_DIR  Install directory (default: %LOCALAPPDATA%\discover)

[CmdletBinding()]
param(
    [switch]$Force,
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$Repo = if ($env:DISCOVER_REPO) { $env:DISCOVER_REPO } else { "asmrtfm/discover" }
$InstallDir = if ($env:DISCOVER_INSTALL_DIR) { $env:DISCOVER_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "discover" }
$BinDir = Join-Path $InstallDir "bin"

$GithubUrl = "https://github.com"
$ReleasesUrl = "$GithubUrl/$Repo/releases"
$JqLatestUrl = "$GithubUrl/jqlang/jq/releases/latest/download"
$JqWindowsAsset = "jq-windows-amd64.exe"

$Platform = "windows-x86_64"
$ChecksumFile = "SHA256SUMS"
$VersionStamp = Join-Path $InstallDir "VERSION"

# Framework paths owned by the installer (replaced wholesale on upgrade)
$FrameworkPaths = @("lib", "langs", "templates", "examples", "README.md", "VERSION")

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "unsupported architecture: $env:PROCESSOR_ARCHITECTURE (releases are built for x86_64)"
}

function Resolve-LatestTag {
    param([string]$Url)
    # Follow /releases/latest's redirect header instead of the GitHub API —
    # the API is rate-limited to 60 anonymous requests/hour.
    $request = [System.Net.HttpWebRequest]::Create($Url)
    $request.AllowAutoRedirect = $false
    $request.Method = "HEAD"
    $response = $request.GetResponse()
    try {
        $location = $response.Headers["Location"]
    } finally {
        $response.Close()
    }
    if (-not $location) { throw "no release redirect from $Url" }
    return ($location -split "/")[-1]
}

$Tag = if ($Version) { $Version } else { Resolve-LatestTag "$ReleasesUrl/latest" }
if ($Tag -notmatch "^v[0-9]") { throw "could not resolve a release tag (got '$Tag')" }
$BareVersion = $Tag.TrimStart("v")

# ── Idempotency check ───────────────────────────────────────────────
if (-not $Force -and (Test-Path $VersionStamp) -and ((Get-Content $VersionStamp -Raw).Trim() -eq $Tag)) {
    Write-Host "==> discover $Tag already installed — up to date"
    exit 0
}

# ── Download and verify ─────────────────────────────────────────────
$WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("discover-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $WorkDir | Out-Null

try {
    $BinariesArchive = "ast-grep-$BareVersion-$Platform.zip"
    $FrameworkArchive = "discover-$BareVersion.tar.gz"
    $DownloadUrl = "$ReleasesUrl/download/$Tag"

    Write-Host "==> Downloading discover $Tag for $Platform"
    foreach ($Asset in @($BinariesArchive, $FrameworkArchive, $ChecksumFile)) {
        Invoke-WebRequest -Uri "$DownloadUrl/$Asset" -OutFile (Join-Path $WorkDir $Asset)
    }

    Write-Host "==> Verifying checksums"
    $Sums = @{}
    foreach ($Line in Get-Content (Join-Path $WorkDir $ChecksumFile)) {
        if ($Line -match "^([0-9a-fA-F]{64})\s+\*?(.+)$") {
            $Sums[$Matches[2].Trim()] = $Matches[1].ToLower()
        }
    }
    foreach ($Asset in @($BinariesArchive, $FrameworkArchive)) {
        if (-not $Sums.ContainsKey($Asset)) { throw "$Asset not listed in $ChecksumFile" }
        $Actual = (Get-FileHash -Algorithm SHA256 (Join-Path $WorkDir $Asset)).Hash.ToLower()
        if ($Actual -ne $Sums[$Asset]) { throw "checksum mismatch for $Asset" }
    }

    # ── Install ─────────────────────────────────────────────────────
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null

    Expand-Archive -Path (Join-Path $WorkDir $BinariesArchive) -DestinationPath $WorkDir -Force
    $BinariesDir = Join-Path $WorkDir "ast-grep-$BareVersion-$Platform"
    foreach ($Exe in @("ast-grep.exe", "sg.exe")) {
        Copy-Item (Join-Path $BinariesDir $Exe) (Join-Path $BinDir $Exe) -Force
    }

    # tar.exe ships with Windows 10 1803+
    tar -xzf (Join-Path $WorkDir $FrameworkArchive) -C $WorkDir
    if ($LASTEXITCODE -ne 0) { throw "failed to extract $FrameworkArchive" }
    $FrameworkDir = Join-Path $WorkDir "discover-$BareVersion"

    foreach ($Path in $FrameworkPaths) {
        $Old = Join-Path $InstallDir $Path
        if (Test-Path $Old) { Remove-Item $Old -Recurse -Force }
        Copy-Item (Join-Path $FrameworkDir $Path) $Old -Recurse -Force
    }
    Copy-Item (Join-Path $FrameworkDir "bin\discover") (Join-Path $BinDir "discover") -Force

    if (-not (Get-Command jq -ErrorAction SilentlyContinue) -and -not (Test-Path (Join-Path $BinDir "jq.exe"))) {
        Write-Host "==> jq not found — installing static jq"
        Invoke-WebRequest -Uri "$JqLatestUrl/$JqWindowsAsset" -OutFile (Join-Path $BinDir "jq.exe")
    }

    # ── User PATH (idempotent) ──────────────────────────────────────
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (($UserPath -split ";") -notcontains $BinDir) {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$BinDir", "User")
        Write-Host "==> Added $BinDir to your user PATH (restart your shell to pick it up)"
    }

    Write-Host "==> Installed discover $Tag"
    Write-Host "    $BinDir\ast-grep.exe"
    Write-Host "    $BinDir\sg.exe"
    Write-Host "    $InstallDir\"
    Write-Host ""
    Write-Host "NOTE: the 'discover' CLI is a bash script — run it from Git Bash or WSL:"
    Write-Host "    bash `"$BinDir\discover`" langs"
} finally {
    Remove-Item $WorkDir -Recurse -Force -ErrorAction SilentlyContinue
}
