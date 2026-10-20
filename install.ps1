# Installs moai on Windows from a GitHub release. install.sh's mirror (moai-j9gv.ukd).
#
#   Invoke-WebRequest -UseBasicParsing -OutFile install.ps1 https://raw.githubusercontent.com/buzzni/moai/main/install.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1 -Version v0.17.0 -Dir C:\tools
#
# **This file is ASCII only.** Windows PowerShell 5.1 reads a script without a BOM in the
# system code page, so a UTF-8 Korean string turns into mojibake there -- and some of those
# bytes decode to curly quotes, which PowerShell takes as string delimiters. A BOM would fix
# 5.1 but gets lost on the way (editors, `git diff`, a copy through the clipboard). A test
# holds this line (`install_ps1_is_ascii`).
#
# **Download it, then run it -- do not pipe it into `iex`.** `exit` inside `iex` closes the
# person's own window, and the refusal lines below hand back a command to run as a file.
#
# **The same line upgrades** (moai-8rmw). When what stands at the install place is this moai,
# it is replaced without -Force and the line says how the version moved. The same version is
# not fetched again, and a version ahead of the latest release is only stepped down when
# -Version names one. -Force is for overwriting someone else's binary, or fetching the same
# version again.
#
# **The branch the receiver steps on is `main`.** Releases are cut from `main`; `develop` holds
# what has not shipped yet.
#
# **Nothing is installed that SHA256SUMS does not vouch for** (moai-j54h). There is no way to
# skip the check -- a check that can be skipped is a check nobody runs.
#
# **A running moai.exe is stepped aside, not overwritten** (moai-j9gv.3fl). Windows refuses to
# write over an image that is running but lets it be renamed, so the old binary becomes
# `moai.exe.old` and the new one takes its name. `moai update` runs this script over the very
# binary that called it; that `.old` is removed right away when nothing holds it, and otherwise
# by the next run of this script.
#
#   -Version <tag>   the release to fetch. Without it, the latest
#   -Dir <dir>       where to install. Without it, %LOCALAPPDATA%\Programs\moai
#   -Force           overwrite someone else's moai.exe there; fetch the same version again
#   -PrintTarget     print this machine's target name and stop
[CmdletBinding()]
param(
  [string]$Version = $env:MOAI_VERSION,
  [string]$Dir = $env:MOAI_INSTALL_DIR,
  [switch]$Force,
  [switch]$PrintTarget,
  [switch]$Help
)

$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1 draws a progress bar per received block, which makes a 10MB
# download take minutes.
$ProgressPreference = 'SilentlyContinue'

function Say([string]$msg) { Write-Host "moai: $msg" }
function Die([string]$msg) {
  [Console]::Error.WriteLine("moai: $msg")
  exit 1
}

if ($Help) {
  @'
Installs moai from a GitHub release. Nothing is installed that SHA256SUMS does not vouch for.

  -Version <tag>   the release to fetch. Without it, the latest
  -Dir <dir>       where to install. Without it, %LOCALAPPDATA%\Programs\moai
  -Force           overwrite someone else's moai.exe there; fetch the same version again
  -PrintTarget     print this machine's target name and stop

When this moai already stands there, it is upgraded without -Force.

  powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1 -Dir C:\tools
'@
  exit 0
}

$repo = if ($env:MOAI_REPO) { $env:MOAI_REPO } else { 'buzzni/moai' }
$base = if ($env:MOAI_BASE_URL) { $env:MOAI_BASE_URL } else { "https://github.com/$repo/releases/download" }
$api = if ($env:MOAI_API_URL) { $env:MOAI_API_URL } else { "https://api.github.com/repos/$repo/releases/latest" }

# Only what the receiver **gave** -- by argument or by environment. The refusal's command
# carries only these: a default written into it would pin this machine's paths into a line
# that gets copied to another.
$givenVersion = $Version
$givenDir = $Dir

# **`MOAI_TARGET` skips the table** (moai-utya) -- the tests build a fake release and measure
# the install path, on any machine. **Only while pointing at a fake release**: on the path a
# receiver walks, a stray name would fetch another platform's binary.
if ($env:MOAI_TARGET -and $env:MOAI_BASE_URL) {
  if ($env:MOAI_TARGET -match '[\\/]|\.\.') { Die "MOAI_TARGET carries a path piece -- $($env:MOAI_TARGET)" }
  $target = $env:MOAI_TARGET
} else {
  # A 32-bit PowerShell on 64-bit Windows reads x86 here and AMD64 in the W6432 one.
  # An x64 PowerShell emulated on ARM64 Windows reads AMD64 and gets the x64 build, which runs
  # there under emulation too -- a native ARM64 PowerShell reads ARM64.
  $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
  switch ($arch) {
    'AMD64' { $target = 'x86_64-pc-windows-msvc' }
    'ARM64' { $target = 'aarch64-pc-windows-msvc' }
    default { Die "no build for this machine ('$arch'). Released are x86_64-pc-windows-msvc, aarch64-pc-windows-msvc here and x86_64-unknown-linux-musl, aarch64-apple-darwin through install.sh. Building from source is in the README" }
  }
}

if ($PrintTarget) {
  Write-Output $target
  exit 0
}

if (-not $Dir) {
  if (-not $env:LOCALAPPDATA) { Die 'no install place -- LOCALAPPDATA is not set. Pass -Dir or MOAI_INSTALL_DIR' }
  $Dir = Join-Path $env:LOCALAPPDATA 'Programs\moai'
}

# Windows PowerShell 5.1 starts with TLS 1.0 and GitHub refuses it.
try {
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {}

# `file://` is read off the disk. Invoke-WebRequest on PowerShell 7 does not take that
# scheme, and the tests point at a fake release there -- curl takes it in install.sh.
function Fetch([string]$url, [string]$out) {
  try {
    $u = [Uri]$url
    if ($u.IsFile) {
      Copy-Item -LiteralPath $u.LocalPath -Destination $out
    } else {
      Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $out
    }
    return $true
  } catch {
    return $false
  }
}

function Digest([string]$path) {
  (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
}

# **Look at the install place before fetching** (moai-8rmw.2mo). Fetching what will be refused
# only spends the receiver's time.
try {
  New-Item -ItemType Directory -Force -Path $Dir | Out-Null
} catch {
  Die "could not make $Dir"
}
$dest = Join-Path $Dir 'moai.exe'
if (Test-Path -LiteralPath $dest -PathType Container) { Die "$dest is a directory -- not overwriting. Pass -Dir to install elsewhere" }

$work = Join-Path ([IO.Path]::GetTempPath()) ("moai-install-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
# `$half` is the copy placed next to the destination before it takes the name. It lives
# outside `$work`, so it is cleaned up apart -- a full disk or a Ctrl-C would otherwise leave a
# cut file in a directory on PATH.
$half = $null

# Runs `$exe $argline` with stdin closed and a ten second limit. Returns its stdout, or $null
# when it did not run, did not end in time or did not end in 0.
#
# **stdin is closed**: an answer that reads stdin would otherwise wait forever.
# **The limit kills**: a child that ignores a polite stop still ends inside it.
function Ask([string]$exe, [string]$argline) {
  try {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = $argline
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [Diagnostics.Process]::Start($psi)
    $proc.StandardInput.Close()
    $out = $proc.StandardOutput.ReadToEndAsync()
    $null = $proc.StandardError.ReadToEndAsync()
    if (-not $proc.WaitForExit(10000)) {
      try { $proc.Kill() } catch {}
      return $null
    }
    # A grandchild that holds the pipe open would keep ReadToEnd waiting past the exit.
    if (-not $out.Wait(2000)) { return $null }
    if ($proc.ExitCode -ne 0) { return $null }
    return $out.Result
  } catch {
    return $null
  }
}

# The version this moai answers with (`0.17.0`), or $null when it is not this moai.
#
# **`--version` alone cannot tell.** The old moai project's binary answers in the same shape,
# and overwriting it breaks that repository's hooks. So it asks one more thing only this moai
# answers -- the first line of `merge-driver --help`, the same since 0.1.0
# (`the_real_binary_is_recognised_as_this_moai` holds it).
function Ours([string]$path) {
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
  $said = Ask $path '--version'
  if ($null -eq $said) { return $null }
  $said = $said.Trim()
  if ($said -cnotmatch '^moai [0-9]') { return $null }
  $said = $said.Substring(5)
  if ($said -cmatch '[^0-9A-Za-z.+-]') { return $null }
  $help = Ask $path 'merge-driver --help'
  if ($null -eq $help -or -not $help.Contains('Merges issues.jsonl per issue')) { return $null }
  return $said
}

# Compares two versions -- 'lt', 'eq', 'gt'. The first three parts as numbers; when they match
# and the text differs (`0.1.4-rc1` and `0.1.4`), 'ne' -- better not to say than to say wrong.
function Order([string]$a, [string]$b) {
  if ($a -eq $b) { return 'eq' }
  $x = $a -split '[.+-]'
  $y = $b -split '[.+-]'
  for ($i = 0; $i -lt 3; $i++) {
    $p = 0; $q = 0
    if ($i -lt $x.Count) { [void][int]::TryParse($x[$i], [ref]$p) }
    if ($i -lt $y.Count) { [void][int]::TryParse($y[$i], [ref]$q) }
    if ($p -lt $q) { return 'lt' }
    if ($p -gt $q) { return 'gt' }
  }
  return 'ne'
}

# Quotes a value so PowerShell reads it back as given. Plain values stay bare -- quoting
# everything makes the common line hard to copy. `,` and `@` are quoted: in an argument `a,b`
# is an array that reaches a program as two words, and a leading `@` splats.
function Quote([string]$v) {
  if ($v -match '^[A-Za-z0-9_./:\\%+=-]+$') { return $v }
  return "'" + $v.Replace("'", "''") + "'"
}

# **A refusal hands over the command to run** (moai-8rmw.t8y), carrying the -Dir and -Version
# the receiver gave -- without them the line installs another version somewhere else. A repo
# other than the default needs MOAI_REPO too, or the script fetches from the default one.
function Refuse([string]$what) {
  $again = ' -Force'
  if ($givenDir) { $again += ' -Dir ' + (Quote $givenDir) }
  if ($givenVersion) { $again += ' -Version ' + (Quote $givenVersion) }
  $from = ''
  # The value is always quoted: the right side of an assignment is a command position, so a
  # bare `fork/moai` runs a command of that name, the variable stays unset, and the line then
  # installs from the default repo with -Force.
  if ($repo -ne 'buzzni/moai') { $from = '$env:MOAI_REPO=''' + $repo.Replace("'", "''") + '''; ' }
  [Console]::Error.WriteLine("moai: $what -- not overwriting. Pass -Dir to install elsewhere")
  [Console]::Error.WriteLine('moai: to overwrite, pass -Force. Run this as is:')
  [Console]::Error.WriteLine('')
  [Console]::Error.WriteLine("  ${from}powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1$again")
  exit 1
}

# Another moai earlier on PATH means this one does not run. Said, not blocked -- which one to
# use is the person's call. **PATH is never edited** -- that is the person's machine.
function OnPath([string]$which) {
  $found = Get-Command moai -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($null -eq $found) {
    [Console]::Error.WriteLine("moai: $Dir is not on PATH -- add it to run moai by name")
    return
  }
  $here = [IO.Path]::GetFullPath($dest)
  $there = [IO.Path]::GetFullPath($found.Source)
  if (-not [string]::Equals($here, $there, [StringComparison]::OrdinalIgnoreCase)) {
    [Console]::Error.WriteLine("moai: on PATH, $there comes first -- $which $dest")
  }
}

try {
  # **Never overwrite in silence** -- unless it is this moai, where overwriting is upgrading
  # (moai-8rmw.lj5). What cannot be told apart is read as someone else's.
  $had = $null
  if ((Test-Path -LiteralPath $dest) -and -not $Force) {
    $item = Get-Item -LiteralPath $dest -Force
    # A symbolic link or junction points elsewhere. `HardLink` is not that -- it is a file with a
    # second name, which stepping aside replaces just as install.sh's `mv` does (install.sh only
    # refuses `-L`). Reparse points as such are not the test: OneDrive's cloud files carry one.
    if ($item.LinkType -and $item.LinkType -ne 'HardLink') { Refuse "$dest is a link to another place" }
    $had = Ours $dest
    if ($null -eq $had) { Refuse "$dest is not this moai" }
  }

  if (-not $Version) {
    Say 'asking for the latest release'
    $latest = Join-Path $work 'latest.json'
    if (-not (Fetch $api $latest)) { Die 'could not read the latest release -- pass -Version with a tag' }
    $m = [regex]::Match((Get-Content -Raw -LiteralPath $latest), '"tag_name"\s*:\s*"([^"]*)"')
    if (-not $m.Success -or -not $m.Groups[1].Value) { Die 'no tag in the release answer -- pass -Version with a tag' }
    $Version = $m.Groups[1].Value
  }
  $bare = $Version -replace '^v', ''

  $way = $null
  if ($null -ne $had) {
    $way = Order $had $bare
    if ($way -eq 'eq') {
      Say "$dest is already $bare -- not fetching"
      OnPath 'the one here is'
      exit 0
    }
    # **Stepping down only when a version was given** -- a bare line filled the version from
    # the latest release and must not undo a build that is ahead of it.
    if ($way -eq 'gt' -and -not $givenVersion) {
      Say "$dest is $had, ahead of the latest release $bare -- not fetching. To step down, pass -Version $Version"
      OnPath 'the one here is'
      exit 0
    }
  }

  # **A place that cannot be written ends before fetching too.**
  $probe = Join-Path $Dir (".moai-write-" + [guid]::NewGuid().ToString('N'))
  try {
    [IO.File]::Create($probe).Dispose()
    Remove-Item -LiteralPath $probe -Force
  } catch {
    Die "cannot write to $Dir -- run as a user who can, or pass -Dir with another place"
  }

  # No `$way` means nothing stood there, or -Force -- there is no version to compare.
  if ($way -eq 'lt') {
    Say "upgrading $dest from $had to $bare"
  } elseif ($way -eq 'gt') {
    Say "stepping $dest down from $had to $bare"
  } elseif ($null -ne $way) {
    Say "changing $dest from $had to $bare"
  }

  $name = "moai-$Version-$target"
  $zip = Join-Path $work "$name.zip"
  $sums = Join-Path $work 'SHA256SUMS'
  Say "fetching $repo $Version $target"
  if (-not (Fetch "$base/$Version/$name.zip" $zip)) { Die "could not fetch the build -- $base/$Version/$name.zip" }
  if (-not (Fetch "$base/$Version/SHA256SUMS" $sums)) { Die 'could not fetch SHA256SUMS -- nothing is installed that cannot be checked' }

  # File names are compared whole, not as a pattern -- a `.` in the tag would match any
  # character. Only the leading `*` of the binary form is dropped.
  $want = $null
  foreach ($line in Get-Content -LiteralPath $sums) {
    $parts = $line.Trim() -split '\s+', 2
    if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq "$name.zip") {
      $want = $parts[0].ToLowerInvariant()
      break
    }
  }
  if (-not $want) { Die "SHA256SUMS has no line for $name.zip -- nothing is installed that cannot be checked" }
  $have = Digest $zip
  if ($want -ne $have) {
    [Console]::Error.WriteLine('moai: what came is not what SHA256SUMS says -- not installing')
    [Console]::Error.WriteLine("  written   $want")
    [Console]::Error.WriteLine("  received  $have")
    exit 1
  }
  Say 'checksums match'

  $unpacked = Join-Path $work 'unpacked'
  try {
    Expand-Archive -LiteralPath $zip -DestinationPath $unpacked
  } catch {
    Die 'could not unpack the build'
  }
  $fresh = Join-Path (Join-Path $unpacked $name) 'moai.exe'
  if (-not (Test-Path -LiteralPath $fresh -PathType Leaf)) { Die 'no moai.exe inside the build' }

  $half = Join-Path $Dir "moai.exe.tmp.$PID"
  Copy-Item -LiteralPath $fresh -Destination $half -Force

  # **Step the standing one aside, then take its name** (moai-j9gv.3fl). A `.old` left by an
  # earlier run goes first; when something still runs it, this run's copy gets its own name.
  # Those `.<pid>.old` names are swept here too once nothing holds them -- only `.old` was looked
  # at before, so each one stayed in a directory on PATH for good.
  foreach ($stale in @(Get-ChildItem -LiteralPath $Dir -Force -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^moai\.exe\.[0-9]+\.old$' })) {
    try { Remove-Item -LiteralPath $stale.FullName -Force } catch {}
  }
  $old = $null
  if (Test-Path -LiteralPath $dest) {
    $old = "$dest.old"
    if (Test-Path -LiteralPath $old) {
      try { Remove-Item -LiteralPath $old -Force } catch { $old = "$dest.$PID.old" }
    }
    Move-Item -LiteralPath $dest -Destination $old -Force
  }
  try {
    Move-Item -LiteralPath $half -Destination $dest
  } catch {
    # The way back can fail too; then say where the old one is instead of a bare error.
    if ($old) {
      try { Move-Item -LiteralPath $old -Destination $dest -Force } catch { Die "could not place $dest -- the old one is at $old" }
    }
    Die "could not place $dest"
  }
  $half = $null
  # Nothing runs it unless this is `moai update` -- then it stays until the next run.
  if ($old) { try { Remove-Item -LiteralPath $old -Force } catch {} }

  if ($null -ne $had) {
    Say "changed $dest from $had to $bare"
  } else {
    Say "installed $dest ($Version)"
  }
  OnPath 'the one just installed is'
} finally {
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
  if ($half) { Remove-Item -LiteralPath $half -Force -ErrorAction SilentlyContinue }
}
