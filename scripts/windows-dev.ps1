# Prepares a Windows machine to build, test and develop moai (moai-e8gx).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1 -SkipTest
#
# Run it from a clone. It is safe to run again: what already stands is skipped.
#
# 1. Looks for Git for Windows, the Visual Studio C++ build tools (`ring` builds C
#    with cl.exe) and rustup. On an ARM64 machine it looks for the ARM64 build tools
#    and clang instead (moai-yc8r.tre): there the host target is
#    aarch64-pc-windows-msvc, and `ring` compiles its C with `clang` from PATH.
#    **It installs nothing.** What is missing is printed as a `winget` command (or the
#    directory to add to PATH) for the person to run, and the script stops there.
# 2. `cargo --version` from the clone -- rustup reads rust-toolchain.toml and fetches
#    the pinned toolchain for the host target (x86_64-pc-windows-msvc, or
#    aarch64-pc-windows-msvc on ARM64) by itself.
# 3. `cargo build --release`, `cargo clippy --all-targets --workspace -- -D warnings` and
#    `cargo test --workspace --no-fail-fast`, one line each. Full output goes to
#    target\windows-dev\*.log.
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

# The machine's own architecture, not this shell's: an x64 PowerShell emulated on ARM64
# Windows reads AMD64 in PROCESSOR_ARCHITECTURE, while rustup on that machine builds for
# aarch64-pc-windows-msvc. Win32_Processor answers from the native WMI host (12 is ARM64);
# where it cannot be asked, the environment is the best there is.
$arm64 = $env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64'
try {
  if ((Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | Select-Object -First 1).Architecture -eq 12) {
    $arm64 = $true
  }
} catch {}

# vswhere ships with every Visual Studio installer, Build Tools included.
# On ARM64 the host compiler is the ARM64 one, and `ring` needs clang for that target --
# Visual Studio's own clang component is enough, once its bin directory is on PATH.
$vcTools = if ($arm64) { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
$vcAdd = '--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
if ($arm64) {
  $vcAdd += ' --add Microsoft.VisualStudio.Component.VC.Tools.ARM64 --add Microsoft.VisualStudio.Component.VC.Llvm.Clang'
  Say 'ARM64 machine -- looking for the ARM64 C++ build tools and clang'
}
$installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer'
$vswhere = Join-Path $installer 'vswhere.exe'
$vc = $null
$anyVs = $null
if (Test-Path -LiteralPath $vswhere) {
  $vc = & $vswhere -latest -products * -requires $vcTools -property installationPath
  $anyVs = & $vswhere -latest -products * -property installationPath
}
# winget does nothing for a Visual Studio that already stands, so one that lacks the
# components is told to add them through its own installer instead.
$vcCommand = if ($anyVs) {
  "& `"$(Join-Path $installer 'setup.exe')`" modify --installPath `"$anyVs`" $vcAdd --passive"
} else {
  "winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override `"--quiet --wait $vcAdd`""
}
if ($vc) {
  Say "C++ build tools ok ($vc)"
} else {
  $missing += $vcCommand
}

if ($arm64) {
  if (Get-Command clang -ErrorAction SilentlyContinue) {
    Say 'clang ok'
  } else {
    # Visual Studio installs clang without putting it on PATH. Point at it when it stands.
    $llvm = $null
    if (Test-Path -LiteralPath $vswhere) {
      $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Llvm.Clang -property installationPath
      if ($vs) {
        foreach ($llvmHost in 'ARM64', 'x64') {
          $bin = Join-Path $vs "VC\Tools\Llvm\$llvmHost\bin"
          if (Test-Path -LiteralPath (Join-Path $bin 'clang.exe')) { $llvm = $bin; break }
        }
      }
    }
    if ($llvm) {
      $missing += "add $llvm to PATH (System Properties -> Environment Variables) -- ring builds its C with clang on ARM64"
    } elseif ($vc) {
      # Without the build tools the line above already adds clang along with them.
      $missing += $vcCommand
    }
  }
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
if ((Step 'clippy' 'cargo clippy --all-targets --workspace -- -D warnings') -ne 0) { $failed++ }
if (-not $SkipTest) {
  # --no-fail-fast: without it cargo stops at the first test binary that fails, so a red unit
  # test hides every integration test behind it and the count below comes out short.
  if ((Step 'test' 'cargo test --workspace --no-fail-fast') -ne 0) {
    $failed++
    # The summary lines are what a note on the tracker needs -- one per test binary that failed.
    foreach ($m in Select-String -LiteralPath (Join-Path $logs 'test.log') -Pattern '^test result: FAILED') {
      Say "  $($m.Line)"
    }
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
