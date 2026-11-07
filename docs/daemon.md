# Read and write the tracker through the daemon

`moai daemon` runs a small local server beside the CLI. Another program — an editor plugin, a dashboard, a script — connects to it and asks for the same answers `moai status --json`, `moai ready --json` and the other read commands give, without starting a child process for each question. It can also write: `add`, `mv`, `edit`, `note`, `defer`, `rm` and `link` do what the CLI commands of the same name do.

What it is:

- A local daemon, one per user. It serves every project you registered with `moai project add`.
- An API for the read commands and for the everyday writes ([Write methods](#write-methods)). A write goes through the same code and the same lock as the CLI's, so the files come out the same.
- Reached only through a Unix socket in your own runtime directory, or on Windows a named pipe open to your account alone. It listens on no network port and has no login: whoever can open the socket, which is you, can read and write.

What it is not:

- It is not needed. The CLI still reads and writes `.moai/` directly, behaves exactly as before, and works with no daemon running.
- It is not a replacement for `moai web` or `moai tui`. Those keep their own servers.

## Start, stop and ask

    moai daemon start          serve in the foreground until stopped
    moai daemon stop           ask the running daemon to exit, and wait until it has
    moai daemon status         is one running?

`start` stays in the foreground. There is no background mode, so give the process to systemd or launchd, or end the line with `&`. Once the socket listens, `start` prints one line and then serves:

    daemon listening on /run/user/1000/moai/daemon.socket — pid 4242, moai 1.0.0. moai daemon stop or Ctrl-C stops it

With `--json` that line is `{"started": true, "pid": …, "version": "…", "proto": 1, "socket": "…"}`. A program that launches the daemon can read this line and then connect.

There is one daemon per user. A second `start` never runs a second daemon:

- If the running one is the same version (the version number and `proto` match), `start` says so and exits 0 (`started` is `false` under `--json`).
- If the running one is another version, `start` names both versions, exits non-zero with the code `version_mismatch`, and leaves the running daemon alone. A long-running process does not pick up a replaced binary, so a daemon started before `cargo build` or `moai update` keeps answering as the old version. Restart it through its service manager, or stop and start it again if you started it yourself. See [Upgrade the daemon](#upgrade-the-daemon). `stop` works on a daemon of any version.
- If a daemon died and left its socket, `start` removes the leftover and starts.
- If a file at the socket path is not a socket of yours, `start` refuses and touches nothing.

`stop` exits 0 when it stopped the daemon or when none was running (`stopped` is `false`). `--json` gives `{stopped, pid, socket}`. It works on a daemon of any version and on one that is at its connection cap (see [The protocol](#the-protocol)). It waits up to 10 seconds for the daemon to end; if the daemon does not end in that time, `stop` fails with a timeout. What the daemon does while it ends is in [Stopping a daemon](#stopping-a-daemon).

`status --json` gives:

| Key | Meaning |
|---|---|
| `supported` | `true` on Linux, macOS and Windows. |
| `running` | A daemon answered. |
| `socket` | The socket path. |
| `stale_socket` | A socket stands that no daemon answers. `start` clears it. |
| `pid`, `version`, `proto` | Only while a daemon runs. |
| `same_version` | Only while a daemon runs. `false` means the daemon has another version number or another `proto` than this `moai`: restart it. Only those two are compared, so a rebuild of the same version number reads as the same (backlog `moai-rz18`). |
| `this_version`, `this_proto` | The `moai` you ran. |
| `service` | The service file, only when one is there. See [Run as a user service](#run-as-a-user-service). |

`status` only reads and exits 0 whether or not a daemon runs. Read `running` and `same_version`, not the exit code. It fails (non-zero) when a file at the socket path is not your socket, when the answer is not a daemon's, or when the daemon is so full that it cannot even answer `hello`. The last case is `busy` and the message names the pid.

`start` has the same version rule: it compares the version number and `proto`, so after you rebuild the same version, a running daemon still reads as "the same build" and keeps the old code. Stop it and start it again by hand.

## Run as a user service

A first install of moai registers the daemon as a user service, so it starts when you log in. Do the same yourself, or undo it, with:

    moai daemon install        register it and start it now
    moai daemon uninstall      stop it and remove the registration
    moai daemon status         service names the service file when there is one

The service file is what being registered means. moai keeps no other record of it.

| OS | Service manager | Service file |
|---|---|---|
| Linux | systemd user manager | `~/.config/systemd/user/moai-daemon.service` (under `$XDG_CONFIG_HOME` when that is set; if only your shell sets it, systemd does not look there and registration fails, leaving no file) |
| macOS | launchd user agent | `~/Library/LaunchAgents/com.buzzni.moai-daemon.plist` |

Windows has no registration yet. See [Windows](#windows).

`moai daemon install` starts nothing itself: it hands the daemon to the service manager, then waits up to 5 seconds for the daemon to answer and reports what it found (`running` under `--json`). A daemon that does not answer yet does not fail the command. Running it again keeps the file, starts the service if it has stopped, and reports `installed: false`.

It refuses these and changes nothing:

| Situation | Code |
|---|---|
| A file at that path that moai did not write | `already_exists` |
| A service file that runs another `moai`. Run `moai daemon uninstall`, then install from the one you want | `already_exists` |
| Running under `sudo`, which would make it root's service | `refused` |
| No service manager, or it refused. No file is left behind, and the CLI works without the daemon | `service_failed` |
| Windows | `unsupported` |

`moai daemon uninstall` stops the service and removes its file. With no file it says so and exits 0. It refuses a file moai did not write, and it refuses to run under `sudo`, which would look at root's service instead of yours (`refused`). A daemon you started yourself keeps running, and `moai` itself stays.

A systemd user service stops when your last session ends unless lingering is on (`loginctl enable-linger`). moai does not turn that on for you.

### When an install registers it

`install.sh` runs `moai daemon install` at the end of a first install, which means no `moai` stood in that directory before, when all of these hold:

- You did not pass `--no-service` or set `MOAI_NO_SERVICE=1`.
- Standard output is a terminal. CI, Docker builds and other programs' installers get no service, and the install's output does not mention it. `curl … | sh` in a terminal counts as a terminal.
- You are not root. As root it says so in one line.
- The release has the command. `--version` can install an older one, which is skipped quietly.

If registration fails, the install still succeeds and says so in one line.

Upgrades never change registration: neither `moai update` nor the install line run over an existing `moai`. Only a first install, `moai daemon install` and `moai daemon uninstall` do. So upgrading an existing moai does not give it a service; run `moai daemon install` if you want one. `install.ps1` does not register anything.

### Write the service file yourself

To keep your own settings, write the file yourself. A file without moai's mark (the comment `moai daemon install wrote this file`) is yours: `moai daemon install` and `moai daemon uninstall` leave it alone, and `moai update` does not restart it.

For systemd, save the following as `~/.config/systemd/user/moai-daemon.service`, replacing the executable path with the absolute path to your installed `moai`:

```ini
[Unit]
Description=moai daemon

[Service]
ExecStart=/home/me/.local/bin/moai daemon start
Restart=on-failure
KillMode=mixed

[Install]
WantedBy=default.target
```

Run `systemctl --user daemon-reload`, then `systemctl --user enable --now moai-daemon.service`. Use `systemctl --user status moai-daemon.service` to inspect it and `systemctl --user stop moai-daemon.service` to stop it. `Restart=on-failure` does not restart a clean stop.

On macOS, save this as `~/Library/LaunchAgents/com.example.moai-daemon.plist`, replacing the executable path with the absolute path to your installed binary and the label with one you control:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.example.moai-daemon</string>
    <key>ProgramArguments</key>
    <array><string>/Users/me/.local/bin/moai</string><string>daemon</string><string>start</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
</dict>
</plist>
```

Load it with `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.example.moai-daemon.plist`. Remove it from the user domain with `launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.example.moai-daemon.plist`.

A service does not inherit your shell's environment, and the socket directory depends on it (see [Where the socket is](#where-the-socket-is)). After the service starts, run `moai daemon status` in a terminal: if it says no daemon is running, check the service status and whether the service chose another directory. Set `MOAI_CTL_DIR` to the same directory for the service (`Environment=` in the unit, `EnvironmentVariables` in the plist) and in your shell.

## Upgrade the daemon

1. Run `moai update`. After a successful installation it checks the newly installed binary's daemon status. If a daemon of another version or protocol still runs, and `moai daemon install` registered a service for this binary and that service is running, it restarts the service through the service manager and waits up to 5 seconds for the new version to answer. If the new version has not answered by then, it says so. In every other case it prints a restart notice to stderr (under `--json`, the keys below instead) and restarts nothing. A service you stopped stays stopped, and registration never changes.
2. If you got the notice, restart the daemon through whatever keeps it running. For a service file you wrote from the examples above, run `systemctl --user restart moai-daemon.service`, or `launchctl kickstart -k gui/$(id -u)/com.example.moai-daemon`. For a daemon you started yourself, run `moai daemon stop`, then `moai daemon start` again where it ran before. Do not restart a service with `moai daemon stop`: the daemon exits 0, neither service manager restarts a clean exit, and a `moai daemon start` typed afterwards runs in your terminal, outside the service.
3. Run `moai daemon status` to check the replacement. A daemon rebuilt with the same version and protocol is indistinguishable from the old build by this check, so restart it manually after such a rebuild.

For `moai update --json`, the result includes `"daemon_restarted": true` when it restarted the service and the new version answered. It includes `"daemon_restart": true` when the post-install check identifies a running daemon with a different version or protocol, can read its version and pid, and no restart was confirmed. Both keys are absent on other paths, including a failed update, a failed status check and no running daemon; their absence does not prove that no restart is needed. The update exit status is unchanged by any of this.

## Where the socket is

The socket is the file `daemon.socket` in the same directory `moai ctl` uses:

1. `MOAI_CTL_DIR`, if set.
2. Otherwise `$XDG_RUNTIME_DIR/moai`.
3. Otherwise, on Windows, `moai/run` under `%LOCALAPPDATA%`.
4. Otherwise `moai-<uid>` in the system temporary directory.

moai creates the directory with mode `0700` and refuses to use one that is not yours or that is open to group or others. That directory is what keeps other users out. `moai daemon status --json` prints the exact path in `socket`.

The name does not end in `.sock` on purpose: `moai ctl ls` treats every `*.sock` in that directory as an open window and would try to talk to the daemon as if it were one.

`daemon.lock` stands beside the socket. The daemon holds a lock on it while it serves, and the kernel releases the lock when the process dies. That is how a second `start` knows a daemon is alive, and how a leftover socket is known to be dead. Do not delete either file by hand while a daemon runs.

## The protocol

Messages are [JSON-RPC 2.0](https://www.jsonrpc.org/specification). Each request is one JSON object on one line, ending in a newline. Each reply is one line. A connection can carry many requests; the daemon answers each before it reads the next.

- A line is at most 1 MiB. A longer line is refused with `-32600` and the connection stays usable.
- A request without an `id` is a notification. The daemon runs nothing for it and sends no reply.
- A batch (a JSON array) is refused with `-32600`.
- At most 32 connections are open at once for ordinary use. A connection past the cap goes to one of 4 lifecycle slots. The daemon reads one line from it, waiting at most 2 seconds. It answers `hello` and `daemon.shutdown` there; for any other method it answers `-32000` with `data` `{code: "busy", pid}` and `id` `null`, and closes the connection. Either way that connection serves one request only. So a client that leaks connections cannot lock `moai daemon status`, `start` and `stop` out.
- When the 4 lifecycle slots are full too, the next connection gets the same `busy` line (with the pid) at once, without being read, and is closed. The slots free within 2 seconds. In this state `moai daemon status`, `start` and `stop` fail with the code `busy`, and the message names the pid.
- A connection that says nothing for 10 minutes is closed.
- While the daemon stops, a request on a connection that was already open is not run. It gets `-32000` with `data` `{code: "stopping", pid}` and its own `id`, and the connection is closed. See [Stopping a daemon](#stopping-a-daemon).
- Each request carries its own context in `params`. A connection keeps no state between requests, so `hello` is optional. The only thing the daemon remembers across requests is the snapshots that `changes` compares (see below).

### Check the version with `hello`

    {"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}
    {"id":1,"jsonrpc":"2.0","result":{"pid":4242,"proto":1,"version":"1.0.0"}}

`proto` is the message version the client speaks; this daemon speaks `1`. A client with an older `proto` is answered. A client with a newer one gets error `-32001` with `data` `{code: "unsupported_proto", proto, client_proto, version, pid}`: upgrade the daemon. A `hello` with no integer `proto` is `-32602`.

### Errors

| Code | When |
|---|---|
| `-32700` | The line is not JSON. `id` is `null`. |
| `-32600` | Not a request object, a batch, a line over 1 MiB, or a bad `id`, `jsonrpc`, `method` or `params` shape. |
| `-32601` | No such method. |
| `-32602` | Bad `params`: an unknown key, a wrong type, a value out of range, an unknown `lang` or `tz`, or no `project`. `project`, `actor`, `lang` and `tz` must be strings (or `null`): an object for `actor` is refused, not dropped. |
| `-32603` | Internal error. |
| `-32000` | The command failed the way the CLI would. `error.data.code` is the `code` from the CLI's `--json` error (`not_found`, `bad_status`, `no_actor`, `not_registered`, `not_a_project`, `busy`, …). |
| `-32001` | `hello` with a `proto` newer than the daemon's. |

When the CLI would have printed notes on stderr before failing, they come in `error.data.messages`.

Real replies, from a running daemon:

    {"error":{"code":-32602,"message":"show: params.limit: 0 is out of range (at least 1)"},"id":9,"jsonrpc":"2.0"}
    {"error":{"code":-32602,"message":"ready: params.actor: must be a string"},"id":11,"jsonrpc":"2.0"}
    {"error":{"code":-32602,"message":"ready: unknown param \"colour\""},"id":12,"jsonrpc":"2.0"}
    {"error":{"code":-32000,"data":{"code":"not_registered"},"message":"relative/path is not a registered project — register it with `moai project add`"},"id":13,"jsonrpc":"2.0"}
    {"error":{"code":-32000,"data":{"code":"busy","pid":4242},"message":"too many connections to the daemon; only hello and daemon.shutdown are served"},"id":null,"jsonrpc":"2.0"}

## Methods

Every method takes a `params` object. These keys are known to every method:

| Key | Meaning |
|---|---|
| `project` | Absolute path of a registered project. Required, except for `hello`, `ping`, `project.ls` and `daemon.shutdown`. |
| `actor` | `"Name (email)"`, like `--user`. Otherwise the project's git config (`user.name`, `user.email`) decides. `MOAI_ACTOR` is **not** read: it belongs to the shell that started the daemon, which says nothing about who sent the request. With neither, a write fails with `-32000` and `data.code` `no_actor`, and a read sets nothing apart as yours or someone else's. The daemon never asks a person. |
| `lang` | Words for messages: `en`, `ko`, `zh`, `ja`, `es`. Otherwise the daemon's `MOAI_LANG`, then its user config. |
| `tz` | A time zone name or POSIX rule. Otherwise the daemon's `TZ`, then its system time zone. |

The response does not report the resolved actor. Clients that need to know whose view they receive should supply `actor` explicitly. The daemon's home and configuration environment also determine user settings and status installation notices.

`project` must be a path you registered with `moai project add`, written as an absolute path. A relative path or an unregistered one is `-32000` with `data.code` `not_registered`. A registered directory that has no `.moai` of its own is `not_a_project`; the daemon does not search upward for a parent's tracker. The registered list is read on every request, so a project added while the daemon runs works at once. A linked git worktree is not covered by registering its main checkout: register the worktree path itself (`moai project add <worktree>`) to send requests for it, and they read and write the main checkout's tracker, as the CLI does there.

Any other key a method does not list is refused with `-32602`, naming the key. A misspelt filter would otherwise be dropped silently and return a list that looks right.

| Method | Extra params (all optional) | Same as |
|---|---|---|
| `hello` | `proto: int` (required) | |
| `ping` | none; replies `{}` | |
| `status` | `worktree: bool` | `moai status --json` |
| `ready` | `worktree: bool`, `limit: int >= 1` | `moai ready --json` |
| `prime` | `worktree: bool` | `moai prime --json` |
| `show` | `id: string` (an issue id or a kind), `tree`, `removed`, `worktree`, `reverse: bool`, `fields: string or [string]`, `sort: string`, `group: string`, `limit: int >= 1`, `after: string`, and the filters below | `moai show … --json` |
| `stats` | `by: string or [string]`, `bucket: "day" or "week"`, `last: int 1..1000`, and the filters below | `moai stats --json` |
| `project.ls` | none (`project` is not used) | `moai project ls --json` |
| `changes` | `cursor: string` | no CLI command; see below |
| `daemon.shutdown` | none; replies `{pid}` and the daemon exits | `moai daemon stop` |

The filters for `show` and `stats` are named like the CLI flags with `-` written as `_`: `status`, `tag`, `no_tag`, `epic`, `milestone`, `parent`, `priority`, `assignee`, `since`, `created`, `done` and `filter` take a string, a number or an array of them; `type` and `grep` take a string; `stale` takes an integer; `deferred`, `all` and `archived` take a boolean. A comma inside a string means "or", and an array means "and" — except `assignee`, where an array also means "or". The meaning is the CLI's, because the daemon runs the same command code.

`sort` takes what `moai show --sort` takes (a preset, a field list, or `milestone=…;epic=…;item=…`), and `group` what `--group` takes. A preset that picks a grouping (`people` groups by assignee) yields to a `group` you send, as on the CLI.

`show` has no `raw` or `as_plan`: those flags print text, not a value.

`by` and `bucket` are case-sensitive, as on the CLI: `"STATUS"` is refused with `-32602` (`unknown axis`). A number outside its range is refused with `-32602` and the message names the range, for example `0 is out of range (at least 1)` or `(1 to 1000)`.

The named fields are typed, but the daemon does not parse an argv. Send `{"status": "todo", "tag": ["parser", "bug"]}`, not `["--status", "todo"]`.

### The reply

Every method except `hello`, `ping` and `daemon.shutdown` answers in one envelope:

    {"exit": 0, "messages": ["…"], "partial": false, "value": <what --json prints>}

- `value` is the CLI's `--json` output, byte for byte: the same text, with the keys in the order the CLI prints them. One output line is carried as it is; several lines are carried as an array of those lines. `jq .result.value` (or `-c`) and `moai --json …` print the same JSON, so you can compare them directly.
- `messages` are the lines the CLI would have written to stderr, without colour.
- `partial` is `true` when the CLI would have exited 1 with a partial answer, for example when a torn snapshot line could not be read. Read `value`, and tell the person.
- `exit` is the exit code the CLI would have ended with, always there. For every method served today it is `1` exactly when `partial` is `true`, and `0` otherwise — for example a `mv` with `from` that lost a race carries the row under `stale` in `value`, `partial` `true` and `exit` `1`. It is a key of its own so that a command that picks another code needs no new key: branch on `exit` to know whether the command did all you asked.

A failed command is not an envelope: it is an `-32000` error (see [Errors](#errors)), where the CLI would always have exited 1.

### Write methods

| Method | Params (`id` is required where listed, except on `rm`, which takes `id` or `line`; `to` is required on `mv`) | Same as |
|---|---|---|
| `add` | `title`, `body`, `epic`, `milestone`, `parent`, `status`, `assignee`, `start`, `due`: string; `tag`: string or [string]; `priority`: int; `kind`: `"issue"`, `"epic"`, `"milestone"` or `"backlog"` | `moai add`, or `moai <kind> add` |
| `mv` | `id`: string or [string]; `to`: string (the column); `from`, `msg`: string; `take`: bool | `moai mv <id>… <to>` |
| `edit` | `id`, `title`, `body`, `epic`, `milestone`, `assignee`, `start`, `due`, `if_start`, `if_due`: string; `tag`, `untag`: string or [string]; `priority`, `shift`, `shift_start`, `shift_due`: int; `if_unplanned`: bool | `moai edit` |
| `note` | `id`, `text`: string | `moai note <id> <text>` |
| `defer` | `id`: string or [string]; `undo`: bool; `msg`, `from`: string | `moai defer` |
| `rm` | `id`: string or [string], or `line`: int (give one of the two); `yes`: bool; `match`: string | `moai rm` |
| `link` | `id`: string; `blocks`, `unblocks`: string or [string] | `moai link` |

The reply is the same envelope as for reads. `value` is what the CLI's `--json` prints: the new row for `add`, the journal line for `note`, `{moved, already, missing, stale, …, taken, theirs}` for `mv`, and so on.

How the params are read:

- The names are the CLI's long flags, with `-` written as `_`: `--if-start` is `if_start`, `--match` is `match`. The arguments that have no flag get a name: `id` for the row, `to` for the column `mv` moves it to, `text` for the note.
- A comma inside `tag`, `untag`, `blocks` or `unblocks` splits the value, as on the CLI. An array is the flag given several times. An `id` is never split.
- **Text is text.** `title`, `body`, `msg` and `text` are taken as written. `"-"` is a dash, not "read stdin": a request has no stdin.
- What the CLI refuses before the command runs is `-32602` here, and the message names the key: a missing `id` or `to`, `shift` with `start` or `due`, `if_start`, `if_due` or `if_unplanned` without `start` or `due`, an `if_due` that is not `YYYY-MM-DD` or `none`, a `shift` past ±999999, a `priority` that is not an integer from 0 to 255, `rm` with both `id` and `line`.
- What the command refuses is `-32000` with the CLI's code and message: an unknown id (`not_found`), an unknown column (`bad_status`), a priority of 4 or more, an `add` with no title.
- `add` makes one row. Making rows from a plan is not served: `from`, `dry_run` and `var` are unknown keys. So is `quiet`, which changes nothing under `--json`, and `type`: `kind` is that choice. `note` takes `text` only; on the CLI, `-b` gives the same text another way.

Who writes is `actor`, else the project's git config, else nobody: the write fails with `-32000` and `data.code` `no_actor`, and the message is the CLI's (where it says `--user`, send `actor`). The daemon's own `MOAI_ACTOR` is not read (see the `actor` key above). An `actor` that is not `Name (email)` is `bad_input`.

One setting of the shell that started the daemon still holds for every request, because a request cannot give it: `MOAI_NOW`, the clock. Start the daemon from a shell without it. `MOAI_HERE` is not read for a request, as `MOAI_ACTOR` is not: a request to a registered linked worktree reads and writes the main checkout's tracker, as the CLI does by default, and the commands it hands back for you to type carry no `MOAI_HERE=1`.

**A write on someone else's row is not stopped.** A `mv` into a started column of a row whose assignee is someone else, or nobody, goes through, as it does for a person at a terminal. The CLI's one-line notice comes in `messages`, and `value.theirs` names the row. `take: true` is `--take`: you become the assignee, and a `Taken-over:` note keeps whose it was.

**Hook rule 5 does not stand on this API — words only.** The hook that refuses an agent's `moai mv` on someone else's row reads the agent's shell commands. A request on the socket passes no hook. An agent that writes through the daemon keeps the rule itself: ask the person first, and on a yes send `take: true` with a `msg` that says who said yes.

A write that runs while the daemon stops runs to the end (see [Stopping a daemon](#stopping-a-daemon)).

A real run, with the project path shortened. Request 2, not shown, moved `repo-mzkg` from `todo` to `in_progress`, so request 3 names a column the row has left:

    {"jsonrpc":"2.0","id":1,"method":"add","params":{"project":"/home/me/repo","title":"lexer crash","tag":"parser,bug","priority":1}}
    {"id":1,"jsonrpc":"2.0","result":{"exit":0,"messages":[],"partial":false,"value":{"id":"repo-gxap","title":"lexer crash","status":"todo","priority":1,"tags":["bug","parser"],"assignee":"Kim","assignee_email":"kim@example.com","created_at":"2026-11-04T14:51:14Z","updated_at":"2026-11-04T14:51:14Z","status_since":"2026-11-04T14:51:14Z","kind":"issue"}}}

    {"jsonrpc":"2.0","id":3,"method":"mv","params":{"project":"/home/me/repo","id":"repo-mzkg","to":"review","from":"todo"}}
    {"id":3,"jsonrpc":"2.0","result":{"exit":1,"messages":["moai: repo-mzkg already stands in_progress — not moved"],"partial":true,"value":{"moved":[],"already":[],"missing":[],"stale":[{"id":"repo-mzkg","status":"in_progress"}],"shelved":[],"stands":[],"unblocked":[],"closable":[],"next":[],"taken":[],"theirs":[]}}}

    {"jsonrpc":"2.0","id":4,"method":"mv","params":{"project":"/home/me/repo","id":"repo-b2b3","to":"in_progress"}}
    …"exit":0,"messages":["moai: repo-b2b3 is Lee (lee@example.com)'s — moved, but ask before you pick it up. On a yes: `moai -C /home/me/repo mv repo-b2b3 in_progress --from in_progress --take -m '<who said yes>'`"],…"theirs":[{"id":"repo-b2b3","owner":"theirs","was":"Lee (lee@example.com)"}]}}}

    {"jsonrpc":"2.0","id":5,"method":"note","params":{"project":"/home/me/repo","id":"repo-mzkg","text":"looks like the lexer","actor":"Park (park@example.com)"}}
    {"id":5,"jsonrpc":"2.0","result":{"exit":0,"messages":[],"partial":false,"value":{"ts":"2026-11-04T14:51:15Z","id":"repo-mzkg","kind":"note","by":"Park","by_email":"park@example.com","text":"looks like the lexer"}}}

    {"jsonrpc":"2.0","id":6,"method":"edit","params":{"project":"/home/me/repo","id":"repo-mzkg","title":"parser bug in lexer","shift":2,"due":"+1"}}
    {"error":{"code":-32602,"message":"edit: params.shift: cannot be used with params.due"},"id":6,"jsonrpc":"2.0"}

    {"jsonrpc":"2.0","id":8,"method":"mv","params":{"project":"/home/me/repo","id":"repo-mzkg","to":"doing"}}
    {"error":{"code":-32000,"data":{"code":"bad_status"},"message":"`doing` is not a column. The columns: todo, in_progress, review, done"},"id":8,"jsonrpc":"2.0"}

With no `actor` in a project whose git config names nobody — although the daemon was started with `MOAI_ACTOR` set:

    {"error":{"code":-32000,"data":{"code":"no_actor"},"message":"there is no telling who is doing this — the git user details are not there.\n\n  git config user.name  \"Name\"\n  git config user.email \"email\"\n\nTo hand it over just this once:  --user \"Name (email)\""},"id":1,"jsonrpc":"2.0"}

### Not served

Commands that exist for a person at a terminal or for tooling, and the writes this API does not carry yet, have no method: `due`, `read`, `archive`, `image`, `init`, the `project add|rm|color` writes, `config`, `models`, `writing`, `wiki`, `ctl`, `tui`, `web`, `watch`, `hook`, `skill`, `supervise`, `merge-driver`, `keys` and `update`. Making rows from a plan is not served either: `add --from` (the `add` method refuses `from`) and `backlog promote`. The aliases `issue`, `epic`, `milestone` and `backlog` go through `add` with `kind` and `show` with `type`. A test lists every CLI command either as served or as not served with a reason, so a new command cannot slip through unnoticed.

## Follow changes with `changes`

The daemon sends no push. A client polls `changes` and re-reads what changed.

    {"jsonrpc":"2.0","id":2,"method":"changes","params":{"project":"/home/me/repo","cursor":"…"}}

The reply `value` is `{changed, cursor, removed, reset}`:

- `cursor` is an opaque string. Do not parse it. Hand the one you got back on the next call. It has the form `2.<instance>.<fingerprint>`: the daemon instance that gave it, and a fingerprint of the snapshot that the daemon compared.
- `changed` lists ids of rows that were added, edited, moved, or got a note since the snapshot of your cursor.
- `removed` lists ids that were in that earlier list and are not in the list now.
- Both lists hold ids only, sorted. Read the rows again with `show` to get their content.

### What is compared

The `session` key is excluded from row fingerprints: it changes with the clock and with heartbeats, not with edits. If a client shows it, it must re-read it independently of `changed`. (`archived` is excluded too, but the compared list never carries it: a row that ages past `archive_days` leaves the list and is in `removed`, below.)

When a line of `.moai/issues.jsonl` cannot be read, `changes` retains the previous fingerprint for its ID while no readable row has that ID, and does not report it as removed. If the ID cannot be recovered, all missing IDs are retained until a complete read. Unreadable lines in archive files retain nothing, because archived rows are not in the compared list. Diagnostics appear in `messages`; an unreadable line in `.moai/issues.jsonl` also sets `partial`.

The daemon remembers recent snapshots in memory: per project up to 8, and about 32 MiB for all projects together. When the memory is full, the snapshot used least recently is dropped. A cursor names one remembered snapshot. On each call the daemon reads the project again, compares it with the snapshot of your cursor row by row, and remembers the new one.

- The rows compared are the list `show` gives with `all: true` and no other param, including the derived values (`derived_status`, `derived_epic`, `derived_milestone`, …). So an edit that came in with `git pull`, a merge or a hand edit is named even when its `updated_at` is old. An epic whose column changed because its last member closed is named too, and so is a member whose inherited milestone changed.
- Notes do not change a row, so they are found by id in the journal, whatever their timestamp. A row with a new note is in `changed`. The text of the note goes only into a fingerprint and is never returned.
- `removed` is every id that left the `all: true` list. That includes rows that `moai rm` deleted, **rows moved to the archive** (`moai archive`), and rows that stood in `done` for longer than `archive_days` and so dropped out of the list. A client must not read `removed` as "deleted"; it means "no longer in the list `show` gives with `all: true`". If you need to know which, read the row by id with `show` and `id`, which also finds archived rows.
- Values that only `show` with an `id` gives (`children`, `members`, `journal`, `commits`, `wiki`) are not compared. If you hold such a view of one row, re-read it when the row or a row it names is in `changed`.

The first call has no cursor. It returns `reset: true`, empty lists and a first cursor:

    {"id":1,"jsonrpc":"2.0","result":{"exit":0,"messages":[],"partial":false,"value":{"changed":[],"cursor":"2.45nGrtyfl2RFrAJx.cM72ha603YZB1hBJv5pmCA","removed":[],"reset":true}}}

### What a client does

1. Call `changes` with no cursor. Keep the `cursor`.
2. Read everything you need with `show` (with `all: true`), `status` and so on.
3. Later, call `changes` with the last cursor. If `reset` is `false`: read each id in `changed` again and drop each id in `removed`. Keep the new `cursor`.
4. If `reset` is `true`, both lists are empty on purpose: **keep the new cursor first, then do the full read again** with `show` and `all: true`, then go on from step 3. Taking the cursor before the full read means a write that lands during the read shows up in the next answer; you may see it twice but never miss it.

`reset` happens when:

- there was no cursor, or it is garbled or from an older format;
- the cursor came from another daemon instance, for example before a restart (the memory is gone with the process);
- the snapshot of the cursor was dropped from the memory, for example because more than 8 snapshots of that project were taken since.

A reset also comes with a fresh cursor, so a client never needs to ask twice. A bad cursor is a reset, not an error, but a cursor that is not a string is `-32602`. A project with no tracker is still an error.

An unreadable journal file does not cause a reset: a re-read would not read it either. The daemon names the file in `messages` and sets `partial` to `true` (and `exit` to `1`). The notes in that file are not seen until it can be read; then the ids they belong to show up in `changed`.

A real run, with the answers trimmed to the interesting part. The project has an epic with two issues (`repo-bld6`, `repo-slt3`):

    moai mv repo-bld6 in_progress
    …"value":{"changed":["repo-32tm","repo-bld6"],"cursor":"2.45nGrtyfl2RFrAJx.yFa17qYZbYIcZPwO4oT43Q","removed":[],"reset":false}

    moai note repo-bld6 'looks like a lexer bug'
    …"value":{"changed":["repo-bld6"],"cursor":"2.45nGrtyfl2RFrAJx.O-Afcf1bSAggFEvisvJZog","removed":[],"reset":false}

    moai mv repo-bld6 done ; moai mv repo-slt3 done
    …"value":{"changed":["repo-32tm","repo-bld6","repo-slt3"],"cursor":"2.45nGrtyfl2RFrAJx.LgF1-K8FA0wBIy4nOK_YEw","removed":[],"reset":false}

    moai rm repo-slt3
    …"value":{"changed":[],"cursor":"2.45nGrtyfl2RFrAJx.iDgf_AeVT3a81w_pVUXbKQ","removed":["repo-slt3"],"reset":false}

    a garbled cursor
    …"value":{"changed":[],"cursor":"2.45nGrtyfl2RFrAJx.iDgf_AeVT3a81w_pVUXbKQ","removed":[],"reset":true}

The epic `repo-32tm` is in the first and third answer although its own line was not edited by hand: its column is a derived value.

## Stopping a daemon

`moai daemon stop` sends the method `daemon.shutdown`. Then the daemon:

1. Removes its socket file and stops listening. From this moment `moai daemon status` says no daemon is running (exit 0), and a new `moai daemon start` waits for this daemon to end.
2. Finishes the requests it was running. A write runs to the end, however long it takes. A read gets up to 5 seconds; a read that is still running then is cut, and nothing is lost.
3. Ends.

A request that comes in on an open connection after step 1 is not run. It gets `-32000` with `data` `{code: "stopping", pid}` and its own `id`. When a write gets this answer, it did not change the tracker: send it again to a new daemon, or run the CLI. A client that connects in the same instant as step 1 can be closed with no answer at all. A connection that was running a request when the stop began closes after that answer, so a further request on it gets only the end of the connection; that request did not run either.

`stop` waits up to 10 seconds for the daemon to end. A write that takes longer makes `stop` fail with `timeout`; the daemon still ends after the write. A `start` run while a daemon stops waits up to 5 seconds. If the old daemon takes longer, `start` fails with "another daemon holds … but does not answer yet": run it again.

The accept loop looks at the stop flag every 0.5 second, so the daemon ends even when its own wake-up connection fails.

Ctrl-C (SIGINT) and SIGTERM stop the daemon the same way: the steps above, then it releases `daemon.lock` and exits 0. So `kill <pid>`, `systemctl stop` and `launchctl stop` end it cleanly, and a running write still runs to the end. A SIGINT that was ignored when the daemon started, as a script's `moai daemon start &` ignores it, stays ignored, so a Ctrl-C meant for the script does not stop the daemon.

Three things still end the daemon in the middle of a write. That write can lose its journal line or leave a temp file. `issues.jsonl` stays whole, and the next `start` clears a socket left behind.

- **A second signal** ends the process at once. A wrapper that forwards signals (npm, foreman, a shell `trap`) can send one right after the terminal's Ctrl-C, so do not run the daemon under one.
- **SIGHUP** has no handler. Closing the terminal of a daemon running in the foreground sends it, so a daemon that outlives its window belongs under a service manager.
- **A service manager's stop timeout.** systemd kills the daemon after 90 seconds by default (`TimeoutStopSec`). launchd's limit is shorter, commonly 20 seconds (`ExitTimeOut`).

Ctrl-C reaches the whole foreground process group, so it also ends a `git` that a running request started. If that `git` was reading who the request is from, a write with no `actor` fails with `no_actor` and a read sets nothing apart as someone else's: send it again. The systemd example sets `KillMode=mixed` for this reason, so that SIGTERM goes to the daemon alone.

One case has no tool path yet (backlog `moai-69bs`): if the socket file was removed while the daemon runs, new clients cannot find it. `moai daemon stop` says "no daemon is running", `status` says the same, and `moai daemon start` waits 5 seconds and fails with "another daemon holds … but does not answer yet", because the daemon still holds `daemon.lock`. A client that is still connected can send `daemon.shutdown` on its open connection. Otherwise end the process by its pid (the `start` line printed it), for example with `kill <pid>`.

## Try it with `socat`

    sock=$(moai daemon status --json | jq -r .socket)

    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}' \
      | socat - UNIX-CONNECT:"$sock"
    {"id":1,"jsonrpc":"2.0","result":{"pid":4242,"proto":1,"version":"1.0.0"}}

    moai project add ~/repo
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"ready","params":{"project":"/home/me/repo","limit":3}}' \
      | socat - UNIX-CONNECT:"$sock" | jq '.result.value'

    printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"changes","params":{"project":"/home/me/repo"}}' \
      | socat - UNIX-CONNECT:"$sock" | jq '.result.value'
    {"changed":[],"cursor":"2.45nGrtyfl2RFrAJx.cM72ha603YZB1hBJv5pmCA","removed":[],"reset":true}

`socat -t 1` keeps the connection open a moment longer if you want to send several lines. `nc -U "$sock"` works the same way on systems whose `nc` supports Unix sockets.

## Try it from Python

Standard library only:

```python
import json, socket, subprocess

sock_path = json.loads(subprocess.check_output(
    ["moai", "daemon", "status", "--json"]))["socket"]

s = socket.socket(socket.AF_UNIX)
s.connect(sock_path)
f = s.makefile("rw", encoding="utf-8", newline="\n")

_id = 0
def call(method, **params):
    global _id
    _id += 1
    f.write(json.dumps({"jsonrpc": "2.0", "id": _id,
                        "method": method, "params": params}) + "\n")
    f.flush()
    reply = json.loads(f.readline())
    if "error" in reply:
        e = reply["error"]
        raise RuntimeError(f'{e["code"]} {e["message"]} {e.get("data")}')
    return reply["result"]

project = "/home/me/repo"           # registered with `moai project add`
call("hello", proto=1)

first = call("changes", project=project)          # reset: True, a cursor
cursor = first["value"]["cursor"]
rows = call("show", project=project, all=True)["value"]  # the full read, after the cursor

# later, in a loop with a sleep:
got = call("changes", project=project, cursor=cursor)["value"]
cursor = got["cursor"]
if got["reset"]:
    rows = call("show", project=project, all=True)["value"]   # cursor first, then the full read
else:
    for row_id in got["removed"]:
        pass                                                   # drop it from your copy
    for row_id in got["changed"]:
        row = call("show", project=project, id=row_id)["value"]
```

## Windows

On Windows the daemon runs the same way and answers the same methods. Only the transport differs: a named pipe instead of a Unix socket.

- `daemon.socket` is still a file in the same directory, but an ordinary one. It holds the pipe's name, `\\.\pipe\moai-<your SID>-<hash>`. Its presence means what a socket file means: it goes away on a clean stop, and a killed daemon leaves it behind for the next `moai daemon start` to clear.
- The pipe is open to your account alone, and remote clients are refused. `moai` also checks that the pipe it reaches belongs to you before it sends anything.
- Without `MOAI_CTL_DIR` or `XDG_RUNTIME_DIR` the directory is `moai/run` under `%LOCALAPPDATA%`, one place per account. moai checks neither its owner nor its access list: the pipe's own access list, not the directory, keeps other users out.
- Ctrl-C and Ctrl-Break stop the daemon the way SIGINT does, and a second one ends the process at once. Closing the console window, logging off and shutting down also start a stop, but Windows allows about five seconds, so a long write can be cut.
- There is no half-close on a pipe. A client cannot shut down its writing side and still read; keep the pipe open until the answer arrives.
- No service manager is set up for you, and `moai daemon install` stops with the code `unsupported`. `moai daemon start` stays in the foreground, so run it in a window you keep, or from Task Scheduler at logon.

Connect from PowerShell like this:

```powershell
$socket = (moai daemon status --json | ConvertFrom-Json).socket
$name = (Get-Content -Raw $socket).Substring(9)   # drop the leading \\.\pipe\
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', $name, [System.IO.Pipes.PipeDirection]::InOut)
$pipe.Connect(2000)
$utf8 = New-Object System.Text.UTF8Encoding($false)
$writer = New-Object System.IO.StreamWriter($pipe, $utf8)
$writer.AutoFlush = $true
$reader = New-Object System.IO.StreamReader($pipe, $utf8)
$writer.WriteLine('{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}')
$reader.ReadLine()
$pipe.Dispose()
```

After `moai update` a running daemon keeps the old binary, which the installer has renamed to `moai.exe.old` (`moai.exe.<pid>.old` when an earlier `.old` is still held). Stop it and start it again.

Decided in: moai-wbdh, moai-ou5y, moai-upq5
