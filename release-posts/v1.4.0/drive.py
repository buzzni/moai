#!/usr/bin/env python3
"""Drive `moai tui` in a pty for the v1.4.0 pictures: the file window and the diff pane.
Same mechanics as scripts/site-demo/drive.py; asciinema records the output.

    asciinema rec --headless --window-size 130x36 -c 'release-posts/v1.4.0/drive.py files <repo>' files.cast
"""
import fcntl, os, pty, select, shutil, signal, struct, subprocess, sys, termios, time

SPC, ENTER, ESC, TAB = ' ', '\r', '\x1b', '\t'

def typed(text, gap=0.09):
    return [(gap, ch) for ch in text]

def chord(*keys):
    # a chord's keys come one by one: the SPC menu drops a key that arrives before it has drawn.
    # The v and o groups stay open after a toggle (so toggles can repeat): Esc closes the group, not the window.
    sticky = keys[0] in 'vo'
    return [(0.8, SPC), *[(0.7, k) for k in keys], *([(1.0, ESC)] if sticky else []), (0.8, None)]

def scenes():
    return {
        # the file window: the tree with the cursor on a source file, then changed files only with the diff pane
        'files': [
            (3.0, None), *chord('g', 'f'), (2.5, 'j'), (0.4, 'j'), (0.4, 'j'), (0.8, TAB),
            *[(0.35, 'j') for _ in range(8)], (4.0, None),
            *chord('v', 'c'), (6.0, None),
        ],
        # the issue list with the detail and the diff of the issue under the cursor
        'list': [
            (3.0, '/'), *typed('conflict', 0.12), (1.0, ENTER), (1.2, 'j'), (0.5, 'j'),
            (1.5, None), *chord('v', 'D'), (5.0, None),
        ],
        # a wide window, detail hidden: the diff side by side
        'split': [
            (3.0, '/'), *typed('conflict', 0.12), (1.0, ENTER), (1.2, 'j'), (0.5, 'j'),
            (1.5, None), *chord('v', 'D'), (1.5, None), *chord('v', 'd'), (1.5, None), *chord('o', 's'), (5.0, None),
        ],
    }

def main():
    scene, repo = sys.argv[1], os.path.abspath(sys.argv[2])
    cols, rows = os.get_terminal_size(sys.stdout.fileno()) if sys.stdout.isatty() else (120, 34)
    moai = os.environ.get('MOAI', 'moai')
    moai = os.path.abspath(moai) if os.sep in moai else (shutil.which(moai) or moai)
    os.environ['MOAI'] = moai   # 장면의 뒤쪽 명령(sh)도 같은 바이너리를 부른다
    # 녹화하는 사람의 환경이 화면에 새지 않게: 정체·키·색·말·시간대는 데모의 것으로
    env = dict(os.environ, TZ='UTC', TERM='xterm-256color', MOAI_NO_UPDATE_CHECK='1', COLORTERM='truecolor', MOAI_LANG=os.environ.get('DEMO_LANG', 'en'),
               MOAI=moai, MOAI_CONFIG=os.path.join(repo, '.demo-home', 'config.toml'))
    for k in ('MOAI_NOW', 'MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR'):
        env.pop(k, None)
    if os.environ.get('DEMO_NOW'):
        # seed.sh 가 날을 맞춘 자(GNU date)와 꼴(RFC3339 UTC)로 — moai 는 MOAI_NOW 를 글자 그대로 저장된 시각과 견준다
        env['MOAI_NOW'] = subprocess.run(['date', '-u', '-d', os.environ['DEMO_NOW'], '+%Y-%m-%dT%H:%M:%SZ'],
                                         check=True, capture_output=True, text=True).stdout.strip()
    pid, fd = pty.fork()
    if pid == 0:
        # 크기는 자식이 exec 하기 전에 제 터미널에 건다 — 부모가 나중에 걸면 TUI 가 0×0 을 먼저 읽을 수 있다
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        os.chdir(repo)
        os.execve(moai, [moai, 'tui'], env)
    out = sys.stdout.buffer
    steps = scenes()[scene]

    def pump(seconds):
        end = time.time() + seconds
        while True:
            left = end - time.time()
            if left <= 0:
                return
            r, _, _ = select.select([fd], [], [], left)
            if r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    return
                out.write(data); out.flush()

    for wait, what in steps:
        pump(wait)
        if what is None:
            continue
        if isinstance(what, tuple):
            subprocess.run(['bash', '-c', what[1]], cwd=repo, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        else:
            os.write(fd, what.encode())
    os.write(fd, b'q')
    pump(0.6)
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass

if __name__ == '__main__':
    main()
