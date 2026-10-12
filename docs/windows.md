# Windows

moai ships a Windows build, `x86_64-pc-windows-msvc`, with the C runtime linked
in — `moai.exe` starts on a clean machine without the Visual C++
redistributable. This page covers installing it, upgrading it, and building it
yourself.

## Install

Download the installer, then run it as a file:

```powershell
Invoke-WebRequest -UseBasicParsing -OutFile install.ps1 https://raw.githubusercontent.com/buzzni/moai/main/install.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1
```

Do not pipe it into `iex`. The script ends with `exit`, which inside `iex`
closes your PowerShell window, and its refusals hand back a command that runs it
as a file.

| Flag | What it does |
|---|---|
| `-Dir <dir>` | install there instead of `%LOCALAPPDATA%\Programs\moai` |
| `-Version <tag>` | install that release instead of the latest |
| `-Force` | replace a `moai.exe` it does not recognise as this moai, or fetch the same version again |
| `-PrintTarget` | print this machine's target name and stop |

The environment variables `install.sh` reads work here too: `MOAI_REPO`,
`MOAI_VERSION` and `MOAI_INSTALL_DIR`.

- **Nothing is installed that `SHA256SUMS` does not vouch for.** The installer
  downloads the zip and `SHA256SUMS` from the release and compares them with
  `Get-FileHash`; there is no flag to skip it
- **It upgrades this moai without `-Force`.** To tell, it runs the `moai.exe`
  already there (`--version`, then `merge-driver --help`) with stdin closed and a
  ten-second limit. Another program of that name is refused unless you pass
  `-Force`
- **It does not edit `PATH`.** When the install directory is not on it, it says
  so in one line — add it yourself in System Properties → Environment Variables
- Only x64 Windows (`AMD64`) has a build. Anywhere else the installer stops and
  names the builds there are

## Upgrade

```powershell
moai update                     # the latest release, over the moai.exe you are running
moai update --version v0.17.0   # that release instead
moai update --dry-run           # print what it would fetch and run
```

`moai update` fetches `install.ps1` from the repository's `main` branch, writes
it to a temporary file and runs it with Windows PowerShell —
`-NoProfile -NonInteractive -ExecutionPolicy Bypass -File <that file> -Dir <the directory of moai.exe> -Force`.
The bypass applies to that one process; your machine's execution policy is not
changed, and your profile is not loaded.

Windows will not overwrite a running program, so the installer renames the
running `moai.exe` to `moai.exe.old` and puts the new one in its place. That
`.old` is removed by the next install.

`moai update` stops before fetching anything when the binary is not named
`moai.exe` or when cargo built it. A directory your account cannot write
(`C:\Program Files`) is caught by the installer itself, before it downloads —
Windows ignores the read-only mark on a folder, so moai does not read it.

## Build from source

`scripts/windows-dev.ps1` checks for Git for Windows, the Visual Studio C++
build tools and rustup, prints a `winget` command for whatever is missing, then
builds, lints and tests the clone and installs the merge driver:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1
```

It installs nothing on its own and never edits `PATH`. What it does step by step,
and what to check on a real machine, is in `CONTRIBUTING.md` under "Developing on
Windows".
