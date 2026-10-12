# Prepares a Windows machine to build, test and develop moai (moai-e8gx).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1 -SkipTest
#
# Run it from a clone. It is safe to run again: what already stands is skipped.
#
# 1. Looks for Git for Windows, the Visual Studio C++ build tools (`ring` builds C
#    with cl.exe) and rustup. **It installs nothing.** What is missing is printed as
#    a `winget` command for the person to run, and the script stops there.
# 2. `cargo --version` from the clone -- rustup reads rust-toolchain.toml and fetches
#    the pinned toolchain for the host target (x86_64-pc-windows-msvc) by itself.
# 3. `cargo build --release`, `cargo clippy --all-targets -- -D warnings` and
#    `cargo test`, one line each. Full output goes to target\windows-dev\*.log.
# 4. `moai merge-driver --install --as <the moai.exe it just built>` -- once per
#    clone, as CONTRIBUTING.md asks on every machine.
#
# It never edits PATH, the registry or anyone's config.
#
# **This file is ASCII only**, like install.ps1: Windows PowerShell 5.1 reads a
# script without a BOM in the system code page.
#
#   -SkipBuild    skip step 3 and 4 (only check the tools)
#   -SkipTest     build and clippy, but not cargo test
#   -SkipDriver   do not install the merge driver
[CmdletBinding()]
param(
  [switch]$SkipBuild,
  [switch]$SkipTest,
  [switch]$SkipDriver
)

$root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path -LiteralPath (Join-Path $root 'Cargo.toml'))) {
  [Console]::Error.WriteLine("windows-dev: no Cargo.toml in $root -- run this from a moai clone")
  exit 2
}
Set-Location -LiteralPath $root

function Say([string]$msg) { Write-Host "windows-dev: $msg" }

# --- 1. tools ---------------------------------------------------------------

$missing = @()

if (Get-Command git -ErrorAction SilentlyContinue) {
  Say 'git ok'
} else {
  $missing += 'winget install --id Git.Git -e'
}

# vswhere ships with every Visual Studio installer, Build Tools included.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vc = $null
if (Test-Path -LiteralPath $vswhere) {
  $vc = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}
if ($vc) {
  Say "C++ build tools ok ($vc)"
} else {
  $missing += 'winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
}

if (Get-Command rustup -ErrorAction SilentlyContinue) {
  Say 'rustup ok'
} else {
  $missing += 'winget install --id Rustlang.Rustup -e'
}

if ($missing.Count -gt 0) {
  Say 'missing -- run these, open a new terminal, and run this script again:'
  foreach ($line in $missing) { Write-Host "  $line" }
  exit 1
}

# --- 2. toolchain -----------------------------------------------------------

$cargo = (& cargo --version 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) {
  [Console]::Error.WriteLine("windows-dev: cargo --version failed -- $cargo")
  exit 1
}
Say $cargo

if ($SkipBuild) { exit 0 }

# --- 3. build, clippy, test -------------------------------------------------

$logs = Join-Path $root 'target\windows-dev'
New-Item -ItemType Directory -Force -Path $logs | Out-Null

# Through cmd so cargo's progress on stderr is not turned into PowerShell errors.
function Step([string]$name, [string]$line) {
  $log = Join-Path $logs "$name.log"
  $started = Get-Date
  cmd /c "$line > `"$log`" 2>&1"
  $code = $LASTEXITCODE
  $took = [int]((Get-Date) - $started).TotalSeconds
  if ($code -eq 0) {
    Say ("{0,-7} ok      {1}s" -f $name, $took)
  } else {
    Say ("{0,-7} FAILED  exit {1}, {2}s -- {3}" -f $name, $code, $took, $log)
  }
  return $code
}

$failed = 0
$built = (Step 'build' 'cargo build --release') -eq 0
if (-not $built) { $failed++ }
if ((Step 'clippy' 'cargo clippy --all-targets -- -D warnings') -ne 0) { $failed++ }
if (-not $SkipTest) {
  if ((Step 'test' 'cargo test') -ne 0) {
    $failed++
    # The summary line is what a note on the tracker needs.
    $summary = Select-String -LiteralPath (Join-Path $logs 'test.log') -Pattern '^test result:' | Select-Object -Last 1
    if ($summary) { Say "  $($summary.Line)" }
  }
}

# --- 4. merge driver --------------------------------------------------------

$exe = Join-Path $root 'target\release\moai.exe'
if (-not $SkipDriver) {
  if ($built -and (Test-Path -LiteralPath $exe)) {
    & $exe merge-driver --install --as $exe
    if ($LASTEXITCODE -ne 0) { $failed++ }
  } else {
    Say 'merge driver skipped -- no moai.exe was built'
  }
}

if ($failed -gt 0) {
  Say "$failed step(s) failed. Logs are in $logs"
  exit 1
}
Say "ready. The binary is $exe"
