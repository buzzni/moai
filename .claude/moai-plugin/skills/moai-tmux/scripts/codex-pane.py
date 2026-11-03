#!/usr/bin/env python3
"""Explicit tmux helper. Refuses ambiguous screens; never discovers a target."""
import os, pathlib, re, shlex, subprocess, sys, time

def tmux(*args):
    return subprocess.check_output(['tmux', *args], text=True, stderr=subprocess.STDOUT)

def places():
    # The checkouts of the repository this helper runs in: the main one and its worktrees.
    try:
        out = subprocess.check_output(['git', 'worktree', 'list', '--porcelain'], text=True, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        raise ValueError('run from the repository root: git worktree list failed here')
    found = [os.path.realpath(line[len('worktree '):]) for line in out.splitlines() if line.startswith('worktree ')]
    if not found: raise ValueError('run from the repository root: git worktree list named no checkout')
    return found

def inside(path, roots):
    if not path: return False
    real = os.path.realpath(path)
    return any(real == root or real.startswith(root.rstrip(os.sep) + os.sep) for root in roots)

def ready(pane, expected='Ask Codex to do anything', prefix=False):
    if not re.fullmatch(r'%[0-9]+', pane) or pane == os.environ.get('TMUX_PANE'):
        raise ValueError('invalid or own pane')
    info = tmux('display-message', '-p', '-t', pane, '#{pane_in_mode}\t#{pane_dead}\t#{pane_pid}\t#{pane_current_path}').rstrip('\n').split('\t')
    if len(info) != 4 or info[:2] != ['0', '0'] or not info[2].isdigit():
        raise ValueError('pane missing, dead or in copy mode')
    # Never type into a pane of another repository: its Codex works on someone else's checkout.
    if not inside(info[3], places()):
        raise ValueError('pane is outside this repository')
    pid = int(info[2])
    procs = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,stat=,pgid=,tpgid=,args='], text=True)
    children = {}
    for line in procs.splitlines():
        parts = line.strip().split(None, 5)
        if len(parts) == 6 and parts[0].isdigit() and parts[1].isdigit():
            children[int(parts[0])] = (int(parts[1]), parts[2], parts[3], parts[4], parts[5])
    tree = {pid}
    for _ in range(4):
        tree |= {p for p, (parent, _, _, _, _) in children.items() if parent in tree}
    def native(args):
        try: return pathlib.Path(shlex.split(args)[0]).name == 'codex'
        except (ValueError, IndexError): return False
    if not any(p in tree and not state.startswith('Z') and pgid == tpgid and tpgid != '-1' and native(args) for p, (_, state, pgid, tpgid, args) in children.items()):
        raise ValueError('no live native Codex process')
    raw = tmux('capture-pane', '-p', '-e', '-t', pane)
    screen = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', raw)
    lines = screen.splitlines()
    positions = [i for i, line in enumerate(lines) if re.match(r'^\s*› ', line)]
    if not positions: raise ValueError('composer missing')
    i = positions[-1]
    composer = lines[i].strip()
    wanted = tuple('› ' + e for e in (expected if isinstance(expected, tuple) else (expected,)))
    if not (composer.startswith(wanted) if prefix else composer in wanted):
        raise ValueError('composer is not the expected input')
    # After a paste the composer and the lines under it hold our own assignment; scanning them
    # would refuse a title or a work line that happens to say "select" or "approval".
    nearby = '\n'.join(lines[max(0, i-8):i] if prefix else lines[max(0, i-8):])
    if re.search(r'(?i)\bworking\b|approval|permission|question|select|confirm', nearby):
        raise ValueError('ambiguous active screen')
    return screen

def settle(pane, expected='Ask Codex to do anything', prefix=False, differs=None, wait=3.0):
    # Codex redraws on its own clock, slower under load: look again until the screen agrees or
    # the time is up, then raise the last refusal. One fixed sleep refused a paste that landed later.
    deadline = time.monotonic() + wait
    while True:
        time.sleep(.2)
        try:
            screen = ready(pane, expected, prefix)
            if differs is None or screen != differs:
                return screen
            error = ValueError('clear did not change the screen')
        except ValueError as e:
            error = e
        if time.monotonic() >= deadline:
            raise error

def main():
    if len(sys.argv) < 3 or sys.argv[1] not in ('ready', 'send', 'clear'):
        raise ValueError('usage: codex-pane.py ready|send|clear %pane [file]')
    op, pane = sys.argv[1:3]
    before = ready(pane)
    if op == 'ready': return
    if op == 'send':
        if len(sys.argv) != 4: raise ValueError('send requires a file')
        path = str(pathlib.Path(sys.argv[3]).resolve(strict=True))
        lines = pathlib.Path(path).read_text().splitlines()
        if not lines or len(lines[0]) < 12: raise ValueError('assignment first line too short to verify')
        first = lines[0]
        buffer = 'moai-send-' + pane[1:]
        tmux('load-buffer', '-b', buffer, path)
        try:
            ready(pane)
            tmux('paste-buffer', '-p', '-d', '-b', buffer, '-t', pane)
        except (ValueError, subprocess.SubprocessError):
            # Nothing was pasted: take our own named buffer back off the person's server.
            try: tmux('delete-buffer', '-b', buffer)
            except (OSError, subprocess.SubprocessError): pass
            raise
        # Submission is deliberate and separate from paste; recheck process/mode. Codex folds a
        # long paste (over about 1,000 characters) into `[Pasted Content N chars]`; the box was
        # empty just before the paste, so that placeholder is ours.
        settle(pane, (first[:12], '[Pasted Content '), prefix=True)
        tmux('send-keys', '-t', pane, 'Enter')
    else:
        tmux('send-keys', '-t', pane, '-l', '/clear')
        try:
            settle(pane, '/clear')
        except ValueError as e:
            raise ValueError(f'{e}; /clear may still be typed in {pane}')
        tmux('send-keys', '-t', pane, 'Enter')
        settle(pane, differs=before, wait=5.0)
    print('ok')

if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, subprocess.SubprocessError) as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
