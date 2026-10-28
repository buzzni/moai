# Windows

moai ships two Windows builds, `x86_64-pc-windows-msvc` and
`aarch64-pc-windows-msvc`, with the C runtime linked in — `moai.exe` starts on a clean machine without the Visual C++
redistributable. This page covers installing it, upgrading it, its config,
editor and hooks, and building it yourself.

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
- x64 (`AMD64`) and ARM64 Windows each have a build; 32-bit Windows does not,
  and there the installer stops and names the builds there are. An x64
  PowerShell running emulated on ARM64 Windows reads `AMD64` and installs the
  x64 build, which runs under emulation — run the installer from the native
  ARM64 PowerShell to get the ARM64 build

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

## Config, editor and hooks

moai itself does not need `sh` or `HOME` on Windows; the hooks need Git for
Windows (below).

- **Your user config** is `.config\moai\config.toml` under `HOME`, or under
  `%USERPROFILE%` when `HOME` is not set — so in cmd and PowerShell it is
  `%USERPROFILE%\.config\moai\config.toml`. A shell that sets its own `HOME`
  (MSYS2, Cygwin, Git Bash on a domain account where `HOME` is `H:\`) reads the
  file under that `HOME` instead. As on Unix, two variables come before both
  homes: `MOAI_CONFIG` names the file itself and wins over everything, then
  `XDG_CONFIG_HOME` puts it at `moai\config.toml` under that directory. To use
  one file from every shell, set `MOAI_CONFIG` to a full path with its drive
  letter, such as `C:\Users\you\moai.toml`. `XDG_CONFIG_HOME` counts only when
  it is a full Windows path (`C:\…` or `\\server\share\…`); any other value is
  ignored and the homes above are used
- **The explorer's editor** (`SPC n`) comes from `VISUAL`, then `EDITOR`, then
  `vi` or `nano` if one is on `PATH`
  - Inside Git Bash it starts through `sh -c`, so the value follows shell
    quoting: `EDITOR="'C:/Program Files/Microsoft VS Code/bin/code' --wait"`
  - Elsewhere there is no shell. The value is split on blanks; a first word in
    double quotes is taken whole, which is how a path with spaces is written:
    `$env:EDITOR = '"C:\Program Files\Notepad++\notepad++.exe" -multiInst -nosession'`.
    The program is looked up on `PATH` with `PATHEXT`, so `code --wait` finds
    `code.cmd`
  - The editor gets the temporary file as `C:/…/moai-backlog-<pid>-<n>.md`
    (`//server/share/…` when the temporary directory is on a share). When
    that path holds a space, `'`, `{` or `[` (a profile named `o'brien`), it
    gets only the file name and starts in the temporary directory instead, so
    a relative path in the editor setting is read from there too. Editors
    built on Git's runtime, such as Git's own `nano.exe`, read their command
    line again by their own rules, and a path that needs quoting there can
    arrive changed — with `o'brien` they opened a file that is not there
- **Hooks and the merge driver** write the path of `moai.exe` with forward
  slashes (`C:/Users/you/AppData/Local/Programs/moai/moai.exe`): Git runs the
  merge driver with its own `sh` and Claude Code runs hooks with Git Bash, and
  both read `\` as an escape. The hooks write just `moai` instead when the
  `moai` on `PATH` is this same file; a path you pass to
  `merge-driver --install --as` is kept as you typed it. **The hooks need Git
  for Windows** — without Git Bash, Claude Code runs hooks in PowerShell, which
  cannot run them
- **Codex and Antigravity run hooks through `cmd.exe`, not Git Bash.** On
  Windows both start a hook as `cmd /c "<command>"`, which cannot read the
  POSIX line moai plants for Unix (`>/dev/null` becomes a cmd redirect and the
  hook ends with exit 0 and no output — the rules are off without a sign). So
  Codex's `.codex/hooks.json` carries a second key per hook,
  `commandWindows`, with `<moai> hook <event> --dialect codex || exit 0`
  (the Unix `command` is unchanged), and Antigravity's `.agents/hooks.json`
  has that one line for every OS, because the file is committed and has no
  Windows-only key. If `moai` is missing the line exits 0 quietly. The line
  holds no quotes at all — `agy` passes it to cmd with every `"` turned into
  `\"`, which cmd cannot read — so a `moai.exe` path with a space or a
  character cmd or sh reads (`C:/Program Files/…`, `%`, `&`, `(`) is written
  as plain `moai`, and that `moai` has to be on `PATH`. After
  `moai skill install` Codex asks you to trust the hooks again in `/hooks`; a
  hook file planted by an older moai reads as `stale` in `moai skill status`
  until you run `moai skill install --agent codex` (or `antigravity`) again
- **`moai skill status` names the Git Bash Claude Code will use**
  (`· git bash  C:\Program Files\Git\bin\bash.exe`), looked up as Claude Code
  does: `CLAUDE_CODE_GIT_BASH_PATH`, then the `bin\bash.exe` beside the `git`
  on `PATH`, then `%ProgramFiles%\Git\bin\bash.exe`. When none is found the row
  is a `!` and says so — Claude Code then runs hooks in PowerShell and the rules
  do not stand. Nothing is blocked and the exit code stays 0
- **MSYS2's git works too.** It hands back paths in its own spelling
  (`/c/Users/you/proj`), on the command line and in the `.git` files it writes
  for a linked worktree, and moai reads them as Windows paths: `/c/…` and
  `/cygdrive/c/…` are `C:\…`, `//server/share/…` is `\\server\share\…`, and any
  other `/…` is under the MSYS2 install, found from the `git.exe` first on
  `PATH` (`C:\msys64\usr\bin\git.exe` puts `/` at `C:\msys64`). Mounts you add
  in MSYS2's `/etc/fstab` are not read — a worktree under such a mount is looked
  for under the install instead
- **Claude Code's plugin records** are read from `%USERPROFILE%\.claude`, the
  place `claude` itself uses, even when `HOME` points somewhere else — unless
  `CLAUDE_CODE_PLUGIN_CACHE_DIR` or `CLAUDE_CONFIG_DIR` names another place, as
  for `claude` itself

## Build from source

`scripts/windows-dev.ps1` checks for Git for Windows, the Visual Studio C++
build tools and rustup, prints a `winget` command for whatever is missing (or,
for a Visual Studio already installed, its installer's `modify` command), then
builds, lints and tests the clone and installs the merge driver:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows-dev.ps1
```

On an ARM64 machine it checks for the ARM64 build tools and `clang` instead:
the build there targets `aarch64-pc-windows-msvc`, and the `ring` crate compiles
its C for that target with the `clang` on `PATH`. Visual Studio's clang does not
put itself on `PATH`, so when it is installed the script names the directory to
add. It looks at the machine, so an x64 PowerShell running emulated on ARM64 gets
the same check.

It installs nothing on its own and never edits `PATH`. What it does step by step,
and what to check on a real machine, is in `CONTRIBUTING.md` under "Developing on
Windows".

Decided in: moai-l3mn moai-j9gv moai-uwzg moai-yc8r
