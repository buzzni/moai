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

    daemon listening on /run/user/1000/moai/daemon.socket — pid 4242, moai 0.48.0. moai daemon stop or Ctrl-C stops it

With `--json` that line is `{"started": true, "pid": …, "version": "…", "proto": 1, "socket": "…"}`. A program that launches the daemon can read this line and then connect.

There is one daemon per user. A second `start` never runs a second daemon:

- If the running one is the same version (the version number and `proto` match), `start` says so and exits 0 (`started` is `false` under `--json`).
- If the running one is another version, `start` names both versions, exits non-zero with the code `version_mismatch`, and leaves the running daemon alone. A long-running process does not pick up a replaced binary, so a daemon started before `cargo build` or `moai update` keeps answering as the old version. Run `moai daemon stop`, then `moai daemon start`. `stop` works on a daemon of any version.
- If a daemon died and left its socket, `start` removes the leftover and starts. Ctrl-C also leaves the socket behind; the next `start` clears it.
- If a file at the socket path is not a socket of yours, `start` refuses and touches nothing.

`stop` exits 0 when it stopped the daemon or when none was running (`stopped` is `false`). `--json` gives `{stopped, pid, socket}`. It works on a daemon of any version and on one that is at its connection cap (see [The protocol](#the-protocol)). It waits up to 10 seconds for the daemon to end; if the daemon does not end in that time, `stop` fails with a timeout.

`status --json` gives:

| Key | Meaning |
|---|---|
| `supported` | `false` on Windows. |
| `running` | A daemon answered. |
| `socket` | The socket path. |
| `stale_socket` | A socket stands that no daemon answers. `start` clears it. |
| `pid`, `version`, `proto` | Only while a daemon runs. |
| `same_version` | Only while a daemon runs. `false` means the daemon has another version number or another `proto` than this `moai`: restart it. Only those two are compared, so a rebuild of the same version number reads as the same (backlog `moai-rz18`). |
| `this_version`, `this_proto` | The `moai` you ran. |

`status` only reads and exits 0 whether or not a daemon runs. Read `running` and `same_version`, not the exit code. It fails (non-zero) when a file at the socket path is not your socket, when the answer is not a daemon's, or when the daemon is so full that it cannot even answer `hello`. The last case is `busy` and the message names the pid.

`start` has the same version rule: it compares the version number and `proto`, so after you rebuild the same version, a running daemon still reads as "the same build" and keeps the old code. Stop it and start it again by hand.

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
- At most 32 connections are open at once for ordinary use. A connection past the cap goes to one of 4 lifecycle slots. The daemon reads one line from it, waiting at most 2 seconds. It answers `hello` and `daemon.shutdown` there; for any other method it answers `-32000` with `data` `{code: "busy", pid}` and `id` `null`, and closes the connection. Either way that connection serves one request only. So a client that leaks connections cannot lock `moai daemon status`, `start` and `stop` out.
- When the 4 lifecycle slots are full too, the next connection gets the same `busy` line (with the pid) at once, without being read, and is closed. The slots free within 2 seconds. In this state `moai daemon status`, `start` and `stop` fail with the code `busy`, and the message names the pid.
- A connection that says nothing for 10 minutes is closed.
- Each request carries its own context in `params`. A connection keeps no state between requests, so `hello` is optional. The only thing the daemon remembers across requests is the snapshots that `changes` compares (see below).

### Check the version with `hello`

    {"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}
    {"id":1,"jsonrpc":"2.0","result":{"pid":4242,"proto":1,"version":"0.48.0"}}

`proto` is the message version the client speaks; this daemon speaks `1`. A client with an older `proto` is answered. A client with a newer one gets error `-32001` with `data` `{code: "unsupported_proto", proto, client_proto, version, pid}`: upgrade the daemon. A `hello` with no integer `proto` is `-32602`.

### Errors

| Code | When |
|---|---|
| `-32700` | The line is not JSON. `id` is `null`. |
| `-32600` | Not a request object, a batch, a line over 1 MiB, or a bad `id`, `jsonrpc`, `method` or `params` shape. |
| `-32601` | No such method. |
| `-32602` | Bad `params`: an unknown key, a wrong type, a value out of range, an unknown `lang` or `tz`, or no `project`. `project`, `actor`, `lang` and `tz` must be strings (or `null`): an object for `actor` is refused, not dropped. |
| `-32603` | Internal error. |
| `-32000` | The command failed the way the CLI would. `error.data.code` is the `code` from the CLI's `--json` error (`not_found`, `not_registered`, `not_a_project`, `busy`, …). |
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

`by` and `bucket` are case-sensitive, as on the CLI: `"STATUS"` is refused with `-32602` (`unknown axis`). A number outside its range is refused with `-32602` and the message names the range, for example `0 is out of range (at least 1)` or `(1 to 1000)`.

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

The reply `value` is `{changed, cursor, removed, reset}`:

- `cursor` is an opaque string. Do not parse it. Hand the one you got back on the next call. It has the form `2.<instance>.<fingerprint>`: the daemon instance that gave it, and a fingerprint of the snapshot that the daemon compared.
- `changed` lists ids of rows that were added, edited, moved, or got a note since the snapshot of your cursor.
- `removed` lists ids that were in that earlier list and are not in the list now.
- Both lists hold ids only, sorted. Read the rows again with `show` to get their content.

### What is compared

The daemon remembers recent snapshots in memory: per project up to 8, and about 32 MiB for all projects together. When the memory is full, the snapshot used least recently is dropped. A cursor names one remembered snapshot. On each call the daemon reads the project again, compares it with the snapshot of your cursor row by row, and remembers the new one.

- The rows compared are the list `show` gives with `all: true` and no other param, including the derived values (`derived_status`, `derived_epic`, `derived_milestone`, …). So an edit that came in with `git pull`, a merge or a hand edit is named even when its `updated_at` is old. An epic whose column changed because its last member closed is named too, and so is a member whose inherited milestone changed.
- Notes do not change a row, so they are found by id in the journal, whatever their timestamp. A row with a new note is in `changed`. The text of the note goes only into a fingerprint and is never returned.
- `removed` is every id that left the `all: true` list. That includes rows that `moai rm` deleted, **rows moved to the archive** (`moai archive`), and rows that stood in `done` for longer than `archive_days` and so dropped out of the list. A client must not read `removed` as "deleted"; it means "no longer in the list `show` gives with `all: true`". If you need to know which, read the row by id with `show` and `id`, which also finds archived rows.
- Values that only `show` with an `id` gives (`children`, `members`, `journal`, `commits`, `wiki`) are not compared. If you hold such a view of one row, re-read it when the row or a row it names is in `changed`.

The first call has no cursor. It returns `reset: true`, empty lists and a first cursor:

    {"id":1,"jsonrpc":"2.0","result":{"messages":[],"partial":false,"value":{"changed":[],"cursor":"2.45nGrtyfl2RFrAJx.cM72ha603YZB1hBJv5pmCA","removed":[],"reset":true}}}

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

An unreadable journal file does not cause a reset: a re-read would not read it either. The daemon names the file in `messages` and sets `partial` to `true`. The notes in that file are not seen until it can be read; then the ids they belong to show up in `changed`.

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

`moai daemon stop` sends the method `daemon.shutdown`. The daemon finishes the requests it is running (up to 5 seconds), removes its socket and ends. Its accept loop looks at the stop flag every 0.5 second, so the daemon ends even when its own wake-up connection fails.

One case has no tool path yet (backlog `moai-69bs`): if the socket file was removed while the daemon runs, new clients cannot find it. `moai daemon stop` says "no daemon is running", `status` says the same, and `moai daemon start` waits 5 seconds and fails with "another daemon holds … but does not answer yet", because the daemon still holds `daemon.lock`. A client that is still connected can send `daemon.shutdown` on its open connection. Otherwise end the process by its pid (the `start` line printed it), for example with `kill <pid>`.

## Try it with `socat`

    sock=$(moai daemon status --json | jq -r .socket)

    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":1}}' \
      | socat - UNIX-CONNECT:"$sock"
    {"id":1,"jsonrpc":"2.0","result":{"pid":4242,"proto":1,"version":"0.48.0"}}

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

Windows is not supported yet. The build works, `moai daemon start` and `stop` report `unsupported`, and `moai daemon status` says `supported: false` and exits 0. A named pipe transport is left for later. Everything else in moai runs on Windows as before.
