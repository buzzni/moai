#!/usr/bin/env python3
"""moai tui 를 pty 에 띄우고 정해 둔 키를 시간에 맞춰 흘린다. asciinema 가 이것의 출력을 녹화한다.

    asciinema rec --headless --window-size 120x32 -c 'scripts/site-demo/drive.py tour <저장소>' tour.cast

장면은 아래 SCENES 에 있다. 한 걸음은 (기다릴 초, 보낼 것) 이고, 보낼 것이 ('sh', 명령) 이면 키 대신
그 저장소에서 moai 명령을 돌린다 — TUI 는 파일이 바뀌면 스스로 다시 읽으니, 뒤에서 옮긴 카드가 화면에서 움직인다.
"""
import fcntl, os, pty, select, shlex, signal, struct, subprocess, sys, termios, time

SPC, ENTER, ESC, TAB, BKSP = ' ', '\r', '\x1b', '\t', '\x7f'

def scenes(repo):
    ids = dict(line.split('=', 1) for line in open(os.path.join(repo, '.demo-ids')).read().split())
    moai = os.environ.get('MOAI', 'moai')
    def sh(cmd):
        return ('sh', cmd.format(moai=shlex.quote(moai), **ids))
    SAM = shlex.quote('Sam Rivera (sam@example.com)')
    def typed(text, gap=0.09):
        return [(gap, ch) for ch in text]
    def key(*seq, gap=0.6):
        return [(gap, k) for k in seq]
    return {
        # 목록 → 칸반 → 위키
        'tour': [
            (2.2, TAB), (1.4, 'j'), *key('j', 'j', 'j', 'j', 'j', 'j', gap=0.85),
            (2.0, SPC), (0.8, 'g'), (0.6, 'b'),
            (1.6, SPC), (0.8, 'v'), (0.6, 'd'), (0.5, ESC),
            (1.8, 'l'), *key('l', 'j', 'l', 'j', 'l', gap=0.85),
            (2.2, SPC), (0.8, 'g'), (0.6, 'w'),
            (1.6, 'j'), *key('j', 'j', 'j', gap=0.7), (1.2, ENTER),
            (2.5, TAB), (1.4, TAB), (1.6, ENTER),
            (3.5, None),
        ],
        # 칸반에서 카드 하나가 Backlog 에 생겨 Done 까지. 뒤에서 moai 로 옮기고, TUI 는 파일이 바뀐 것을 보고 다시 그린다
        'lifecycle': [
            (1.8, SPC), (0.7, 'g'), (0.6, 'b'),
            (1.0, SPC), (0.7, 'v'), (0.6, 'd'), (0.5, ESC),
            (2.5, sh("{moai} backlog add 'Pin notes to the top of a notebook' -e {editor} --user " + SAM + " --json | jq -r .id > .demo-card")),
            (3.2, sh("{moai} backlog promote $(cat .demo-card) -e {editor} --from - --user " + SAM + " >/dev/null <<'PLAN'\n- [p1] Pin notes to the top of a notebook\nPLAN\n"
                     "{moai} show -e {editor} -g 'Pin notes' --type issue --json | jq -r '.[0].id' > .demo-card")),
            (4.0, sh("{moai} mv $(cat .demo-card) in_progress --user " + SAM)),
            (4.0, sh("{moai} mv $(cat .demo-card) review --user " + SAM)),
            (4.0, sh("{moai} mv $(cat .demo-card) done --user " + SAM)),
            (5.0, None),
        ],
    }

def main():
    scene, repo = sys.argv[1], os.path.abspath(sys.argv[2])
    steps = scenes(repo)[scene]
    cols, rows = os.get_terminal_size(sys.stdout.fileno()) if sys.stdout.isatty() else (120, 32)
    env = dict(os.environ, TERM='xterm-256color', MOAI_NO_UPDATE_CHECK='1', COLORTERM='truecolor',
               MOAI_CONFIG=os.path.join(repo, '.demo-home', 'config.toml'))
    env.pop('MOAI_NOW', None)
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(repo)
        os.execvpe(os.environ.get('MOAI', 'moai'), [os.environ.get('MOAI', 'moai'), 'tui'], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
    out = sys.stdout.buffer

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
