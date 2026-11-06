4-2. **If you test tmux, do it on a separate server only** — `env -u TMUX tmux -L <unique name>`
   on every call. Put the epic id in the name so it cannot collide with the test servers of
   the workers beside you or of a review subagent. `-S <socket>` works too, but a socket path
   does not stand past the unix limit (about 100 bytes), and a scratchpad path is usually too
   long. Never use `kill-server` or `kill-session` without `-L`/`-S`: inside tmux a bare
   `tmux` goes to the person's default server and kills every session, and `TMUX_TMPDIR` does
   not fence it in. A script that calls `tmux` inside itself cannot be given `-L` by hand, so
   run it with a wrapper at the front of `PATH` that calls the real `tmux` by absolute path
   and inserts `-L`. Do not send keys into a pane someone else raised. If you raise a test
   agent on that server, keep its cwd outside the root (the scratchpad) — raised in the root,
   it stands in `ListAgents` as a session of this repository and a supervisor takes it for a worker.
   **Under cmux claude-teams** (`$TMUX` starts with `/tmp/cmux-`) the `tmux` on `PATH` is cmux's shim,
   which drops `-L`, `-S` and `-f` and so reaches the person's cmux whatever socket you name — call a
   real tmux by absolute path instead (`env -u TMUX <real tmux> -L <unique name> …`; `type -a tmux` lists the candidates, take one whose
   `readlink -f` has no `cmux` in it), and point a wrapper script at that same path. With no real tmux there,
   do not test tmux in that shell.
   **Give a review subagent these words too**

