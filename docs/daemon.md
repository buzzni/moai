# Read the tracker through the daemon

`moai daemon` runs a small local server beside the CLI. Another program — an editor plugin, a dashboard, a script — connects to it and asks for the same answers `moai status --json`, `moai ready --json` and the other read commands give, without starting a child process for each question.

What it is:

- A local daemon, one per user. It serves every project you registered with `moai project add`.
- A read-only API. Writes (`add`, `mv`, `note` and the rest) are not served yet.
- Reached only through a Unix socket in your own runtime directory. It listens on no network port and has no login: whoever can open the socket, which is you, can read.

What it is not:

- It is not needed. The CLI still reads and writes `.moai/` directly, behaves exactly as before, and works with no daemon running.
- It is not a replacement for `moai web` or `moai tui`. Those keep their own servers.
- It does not run on Windows yet. See [Windows](#windows).

## Start, stop and ask

    moai daemon start          serve in the foreground until stopped
    moai daemon stop           ask the running daemon to exit, and wait until it has
    moai daemon status         is one running?

`start` stays in the foreground. There is no background mode, so give the process to systemd or launchd, or end the line with `&`. Once the socket listens, `start` prints one line and then serves:

    daemon listening on /run/user/1000/moai/daemon.socket — pid 4242, moai 0.49.0. moai daemon stop or Ctrl-C stops it

With `--json` that line is `{"started": true, "pid": …, "version": "…", "proto": 1, "socket": "…"}`. A program that launches the daemon can read this line and then connect.

There is one daemon per user. A second `start` never runs a second daemon:

- If the running one is the same build, `start` says so and exits 0 (`started` is `false` under `--json`).
- If the running one is another build, `start` names both versions, exits non-zero with the code `version_mismatch`, and leaves the running daemon alone. A long-running process does not pick up a replaced binary, so a daemon started before `cargo build` or `moai update` keeps answering as the old version. Run `moai daemon stop`, then `moai daemon start`. `stop` works on a daemon of any version.
- If a daemon died and left its socket, `start` removes the leftover and starts. Ctrl-C also leaves the socket behind; the next `start` clears it.
- If a file at the socket path is not a socket of yours, `start` refuses and touches nothing.

`stop` exits 0 when it stopped the daemon or when none was running (`stopped` is `false`). `--json` gives `{stopped, pid, socket}`.

`status --json` gives:

| Key | Meaning |
|---|---|
| `supported` | `false` on Windows. |
| `running` | A daemon answered. |
| `socket` | The socket path. |
| `stale_socket` | A socket stands that no daemon answers. `start` clears it. |
| `pid`, `version`, `proto` | Only while a daemon runs. |
| `same_version` | Only while a daemon runs. `false` means it is another build than this `moai`: restart it. |
| `this_version`, `this_proto` | The `moai` you ran. |

`status` only reads and exits 0 whether or not a daemon runs. Read `running` and `same_version`, not the exit code.

## Where the socket is

The socket is the file `daemon.socket` in the same directory `moai ctl` uses:

1. `MOAI_CTL_DIR`, if set.
2. Otherwise `$XDG_RUNTIME_DIR/moai`.
3. Otherwise `moai-<uid>` in the system temporary directory.

moai creates the directory with mode `0700` and refuses to use one that is not yours or that is open to group or others. That directory is what keeps other users out. `moai daemon status --json` prints the exact path in `socket`.

The name does not end in `.sock` on purpose: `moai ctl ls` treats every `*.sock` in that directory as an open window and would try to talk to the daemon as if it were one.

`daemon.lock` stands beside the socket. The daemon holds a lock on it while it serves, and the kernel releases the lock when the process dies. That is how a second `start` knows a daemon is alive, and how a leftover socket is known to be dead. Do not delete either file by hand while a daemon runs.

## The protocol

Messages are [JSON-RPC 2.0](https://www.jsonrpc.org/specification). Each request is one JSON object on one line, ending in a newline. Each reply is one line. A connection can carry many requests; the daemon answers each before it reads the next.

- A line is at most 1 MiB. A longer line is refused with `-32600` and the connection stays usable.
- A request without an `id` is a notification. The daemon runs nothing for it and sends no reply.
- A batch (a JSON array) is refused with `-32600`.
- At most 32 connections are open at once. The next one gets a single error line (`-32000`, `data.code` is `busy`) and is closed.
- A connection that says nothing for 10 minutes is closed.
- Each request carries its own context in `params`. The daemon keeps no state between requests, so `hello` is optional.

### Check the version with `hello`

    {"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}
    {"jsonrpc":"2.0","id":1,"result":{"proto":1,"version":"0.49.0","pid":4242}}

`proto` is the message version the client speaks; this daemon speaks `1`. A client with an older `proto` is answered. A client with a newer one gets error `-32001` with `data` `{code: "unsupported_proto", proto, client_proto, version, pid}`: upgrade the daemon. A `hello` with no integer `proto` is `-32602`.

### Errors

| Code | When |
|---|---|
| `-32700` | The line is not JSON. `id` is `null`. |
| `-32600` | Not a request object, a batch, a line over 1 MiB, or a bad `id`, `jsonrpc`, `method` or `params` shape. |
| `-32601` | No such method. |
| `-32602` | Bad `params`: an unknown key, a wrong type, a value out of range, an unknown `lang` or `tz`, or no `project`. |
| `-32603` | Internal error. |
| `-32000` | The command failed the way the CLI would. `error.data.code` is the `code` from the CLI's `--json` error (`not_found`, `not_registered`, `not_a_project`, `busy`, …). |
| `-32001` | `hello` with a `proto` newer than the daemon's. |

When the CLI would have printed notes on stderr before failing, they come in `error.data.messages`.

## Methods

Every method takes a `params` object. These keys are known to every method:

| Key | Meaning |
|---|---|
| `project` | Absolute path of a registered project. Required, except for `hello`, `ping`, `project.ls` and `daemon.shutdown`. |
| `actor` | `"Name (email)"`, like `--user`. Otherwise the project's git config decides, as for the CLI. The daemon never asks a person. |
| `lang` | Words for messages: `en`, `ko`, `zh`, `ja`, `es`. Otherwise `MOAI_LANG`, then your user config. |
| `tz` | A time zone name or POSIX rule. Otherwise as the CLI reads it. |

`project` must be a path you registered with `moai project add`, written as an absolute path. A relative path or an unregistered one is `-32000` with `data.code` `not_registered`. A registered directory that has no `.moai` of its own is `not_a_project`; the daemon does not search upward for a parent's tracker. The registered list is read on every request, so a project added while the daemon runs works at once.

Any other key a method does not list is refused with `-32602`, naming the key. A misspelt filter would otherwise be dropped silently and return a list that looks right.

| Method | Extra params (all optional) | Same as |
|---|---|---|
| `hello` | `proto: int` (required) | |
| `ping` | none; replies `{}` | |
| `status` | `worktree: bool` | `moai status --json` |
| `ready` | `worktree: bool`, `limit: int >= 1` | `moai ready --json` |
| `prime` | `worktree: bool` | `moai prime --json` |
| `show` | `id: string` (an issue id or a kind), `tree`, `removed`, `worktree`, `reverse: bool`, `fields: string or [string]`, `sort: string`, `limit: int >= 1`, `after: string`, and the filters below | `moai show … --json` |
| `stats` | `by: string or [string]`, `bucket: "day" or "week"`, `last: int 1..1000`, and the filters below | `moai stats --json` |
| `project.ls` | none (`project` is not used) | `moai project ls --json` |
| `changes` | `cursor: string` | no CLI command; see below |
| `daemon.shutdown` | none; replies `{pid}` and the daemon exits | `moai daemon stop` |

The filters for `show` and `stats` are named like the CLI flags with `-` written as `_`: `status`, `tag`, `no_tag`, `epic`, `milestone`, `parent`, `priority`, `assignee`, `since`, `created`, `done` and `filter` take a string, a number or an array of them; `type` and `grep` take a string; `stale` takes an integer; `deferred`, `all` and `archived` take a boolean. A comma inside a string means "or", and an array means "and" — except `assignee`, where an array also means "or". The meaning is the CLI's, because the daemon runs the same command code.

`show` has no `raw` or `as_plan`: those flags print text, not a value.

The named fields are typed, but the daemon does not parse an argv. Send `{"status": "todo", "tag": ["parser", "bug"]}`, not `["--status", "todo"]`.

### The reply

Every method except `hello`, `ping` and `daemon.shutdown` answers in one envelope:

    {"value": <what --json prints>, "messages": ["…"], "partial": false}

- `value` is the CLI's `--json` output, as a value.
- `messages` are the lines the CLI would have written to stderr, without colour.
- `partial` is `true` when the CLI would have exited 1 with a partial answer, for example when a torn snapshot line could not be read. Read `value`, and tell the person.

### Not served

The write commands, and commands that exist for a person at a terminal or for tooling, have no method: `add`, `mv`, `edit`, `rm`, `note`, `defer`, `link`, `due`, `read`, `archive`, `image`, `init`, the `project add|rm|color` writes, `config`, `models`, `writing`, `wiki`, `ctl`, `tui`, `web`, `watch`, `hook`, `skill`, `supervise`, `merge-driver`, `keys` and `update`. The aliases `issue`, `epic`, `milestone` and `backlog` are `show` with `type`. A test lists every CLI command either as served or as not served with a reason, so a new command cannot slip through unnoticed.

## Follow changes with `changes`

The daemon sends no push. A client polls `changes` and re-reads what changed.

    {"jsonrpc":"2.0","id":2,"method":"changes","params":{"project":"/home/me/repo","cursor":"…"}}

The reply `value` is `{cursor, changed, removed, reset}`:

- `cursor` is an opaque string. Do not parse it. Hand the one you got back on the next call. The daemon stores nothing, so a cursor survives a daemon restart.
- `changed` lists ids of live rows that were added, edited, moved, or got a note since the cursor. A note counts, even though it does not change the row.
- `removed` lists ids that `moai rm` deleted and that no longer exist.
- Both lists hold ids only, sorted. Read the rows again with `show` to get their content.

The first call has no cursor. It returns `reset: true`, empty lists and a first cursor. The loop is:

1. Call `changes` with no cursor. Keep the `cursor`.
2. Read everything you need with `show` or `status`.
3. Later, call `changes` with the last cursor. For each id in `changed`, read the row again; drop each id in `removed`. Keep the new `cursor`.
4. If `reset` is `true`, the lists are empty on purpose: **keep the new cursor first, then do a full read again**, then go on from step 3. Taking the cursor before the full read means a write that lands during the read shows up in the next answer; you may see it twice but never miss it.

A reset also comes with a fresh cursor, so a client never needs to ask twice. `reset` happens when:

- there was no cursor, or it is garbled, from an older format, or from another project;
- rows moved into the archive (`moai archive`);
- the number of rows changed in a way the journal does not explain, for example after a merge or a hand edit;
- the clock went back before the cursor;
- the journal could not be read, or a torn journal line might belong to the window.

A bad cursor is a reset, not an error. A project with no tracker is still an error.

Two writes in the same second, or two edits of one row in the same second, are both reported. The window reaches a couple of seconds back so a write that was slow to commit is not missed; the cursor remembers what it already reported, so the same change is not reported twice.

## Try it with `socat`

    sock=$(moai daemon status --json | jq -r .socket)

    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}' \
      | socat - UNIX-CONNECT:"$sock"
    {"id":1,"jsonrpc":"2.0","result":{"pid":4242,"proto":1,"version":"0.49.0"}}

    moai project add ~/repo
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"ready","params":{"project":"/home/me/repo","limit":3}}' \
      | socat - UNIX-CONNECT:"$sock" | jq '.result.value'

    printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"changes","params":{"project":"/home/me/repo"}}' \
      | socat - UNIX-CONNECT:"$sock" | jq '.result.value'
    {"changed":[],"cursor":"eyJ2…","removed":[],"reset":true}

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
board = call("status", project=project)["value"]  # the full read, after the cursor

# later, in a loop with a sleep:
got = call("changes", project=project, cursor=cursor)["value"]
cursor = got["cursor"]
if got["reset"]:
    board = call("status", project=project)["value"]
else:
    for row_id in got["changed"]:
        row = call("show", project=project, id=row_id)["value"]
```

## Windows

Windows is not supported yet. The build works, `moai daemon start` and `stop` report `unsupported`, and `moai daemon status` says `supported: false` and exits 0. A named pipe transport is left for later. Everything else in moai runs on Windows as before.
