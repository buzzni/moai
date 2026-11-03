#!/usr/bin/env python3
"""Explicit tmux helper. Refuses ambiguous screens; never discovers a target."""
import os, pathlib, re, shlex, subprocess, sys, time

def tmux(*args):
    return subprocess.check_output(['tmux', *args], text=True, stderr=subprocess.STDOUT)

def ready(pane, expected='Ask Codex to do anything', prefix=False):
    if not re.fullmatch(r'%[0-9]+', pane) or pane == os.environ.get('TMUX_PANE'):
        raise ValueError('invalid or own pane')
    info = tmux('display-message', '-p', '-t', pane, '#{pane_in_mode} #{pane_dead} #{pane_pid}').strip().split()
    if len(info) != 3 or info[:2] != ['0', '0']:
        raise ValueError('pane missing, dead or in copy mode')
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
    wanted = '› ' + expected
    if not (composer.startswith(wanted) if prefix else composer == wanted):
        raise ValueError('composer is not the expected input')
    nearby = '\n'.join(lines[max(0, i-8):])
    if re.search(r'(?i)\bworking\b|approval|permission|question|select|confirm', nearby):
        raise ValueError('ambiguous active screen')
    return screen

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
        tmux('load-buffer', '-b', 'moai-send-'+pane[1:], path)
        ready(pane)
        tmux('paste-buffer', '-p', '-d', '-b', 'moai-send-'+pane[1:], '-t', pane)
        time.sleep(.2)
        # Submission is deliberate and separate from paste; recheck process/mode.
        ready(pane, first[:12], prefix=True)
        tmux('send-keys', '-t', pane, 'Enter')
    else:
        tmux('send-keys', '-t', pane, '-l', '/clear')
        time.sleep(.2)
        ready(pane, '/clear')
        tmux('send-keys', '-t', pane, 'Enter')
        time.sleep(.5)
        after = ready(pane)
        if after == before: raise ValueError('clear did not change the screen')
    print('ok')

if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, subprocess.SubprocessError) as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
