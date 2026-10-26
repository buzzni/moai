#!/usr/bin/env python3
"""셸을 pty 에 띄우고 명령을 한 글자씩 쳐 넣는다. asciinema 가 이것의 출력을 녹화한다.

    asciinema rec --headless --window-size 100x30 -c 'scripts/site-demo/shell.py init <저장소>' init.cast

drive.py 는 moai tui 를 띄우지만, 사용 방법 페이지의 장면(init 설정 화면, 백로그를 에픽으로 펼치기)은
셸에서 명령을 치는 모습이라 따로 둔다. 한 걸음은 (기다릴 초, 보낼 것) 이고, 보낼 것이 함수면 보내는 순간에
불러 글을 얻는다 — 앞 명령이 만든 id 를 다음 명령에 쳐 넣으려는 것이다.
"""
import fcntl, json, os, pty, select, shlex, signal, struct, subprocess, sys, termios, time

ENTER, ESC, UP, DOWN, LEFT, RIGHT, CLEAR = '\r', '\x1b', '\x1b[A', '\x1b[B', '\x1b[D', '\x1b[C', '\x0c'

def scenes(repo, env):
    def moai_json(*args):
        out = subprocess.run([env['MOAI'], *args, '--json'], cwd=repo, env=env, capture_output=True, text=True).stdout
        return json.loads(out)
    def typed(text, gap=0.05, then=0.5):
        # 한 줄을 치고 Enter. 사람이 치는 속도로, 끝에서 잠깐 멈춘다
        return [(gap, ch) for ch in text] + [(then, ENTER)]
    def typed_later(make, gap=0.05, then=0.5):
        # 글을 보내는 순간에 만든다. 글자 수를 미리 모르니 한 걸음에 한 줄을 통째로 흘린다
        return [(gap, lambda: ''.join(make()))] + [(then, ENTER)]
    def key(*seq, gap=0.7):
        return [(gap, k) for k in seq]

    def backlog_id():
        rows = [r for r in moai_json('backlog', 'ls') if r.get('title') == 'Export a notebook as Markdown']
        return rows[0]['id']
    def epic_id():
        rows = moai_json('show', '-g', 'Markdown export', '--type', 'epic')
        return rows[0]['id']
    def demo_id(name):
        # seed.sh 가 적어 둔 id (.demo-ids 의 name=id 줄)
        with open(os.path.join(repo, '.demo-ids')) as f:
            return dict(l.strip().split('=', 1) for l in f if '=' in l)[name]

    return {
        # 빈 저장소에서 moai init: 칸을 오가며 고를 것을 보이고, Enter 로 심은 뒤 무엇이 생겼는지 본다
        'init': [
            (1.2, None),
            *typed('moai init', then=1.0),
            (2.0, DOWN), (1.2, RIGHT), (1.2, RIGHT), (1.0, LEFT), (0.8, LEFT),
            (1.4, DOWN), (1.2, RIGHT), (1.4, LEFT),
            *key(DOWN, DOWN, DOWN, DOWN, DOWN, gap=0.9),
            (2.0, ENTER),
            (2.5, None),
            *typed('git status --short', then=1.6),
            *typed('moai status', then=4.0),
        ],
        # 떠오른 것을 백로그에 담고, 때가 되면 에픽과 이슈로 펼친다
        'backlog': [
            (1.2, None),
            *typed("moai backlog add 'Export a notebook as Markdown'", then=1.6),
            *typed('moai backlog ls', then=2.4),
            *typed_later(lambda: f"moai backlog promote {backlog_id()} --from - <<'PLAN'", then=0.4),
            *typed('# Markdown export', then=0.3),
            *typed('- [p1] Render notes to Markdown', then=0.3),
            *typed('- [p2] Copy attachments next to the file', then=0.3),
            *typed('- [p2] Add Export to the notebook menu', then=0.3),
            *typed('PLAN', then=2.4),
            *typed_later(lambda: f'moai show --tree -e {epic_id()}', then=5.0),
        ],
        # 탐색기 키를 내 것으로 (v0.23.0 의 Discord 소식, moai-hwl4). keys.toml 은 record-guide.sh 가 써 둔다 —
        # 바꾼 줄 둘과 부딪히는 줄 하나. `moai keys` 가 바꾼 줄에 * 를 달고 부딪힌 줄을 왜 뺐는지 대는 것,
        # 메뉴가 바꾼 키(SPC x 로 닫기)를 대는 것을 차례로 보인다
        'keys': [
            (1.2, None),
            *typed('cat ~/.config/moai/keys.toml', then=1.8),
            *typed('moai keys | head -22', then=3.5),
            *typed('moai tui', then=3.0),
            (1.0, ' '),
            (3.5, ' '),
            (1.2, None),
            *key(' ', 'x', gap=0.8),
            (1.5, None),
        ],
        # 위키를 찾고, 교훈을 적고, 이슈가 위키를 내미는 것을 본다 (v0.26.0 의 Discord 소식, moai-m7jm). 이미 선 교훈
        # 하나는 record-guide.sh 가 심어 둔다. typed 의 then 은 Enter 앞의 멈춤이라 출력을 볼 틈은 (초, None) 로 따로
        # 둔다 — 안 두면 다음 명령의 첫 글자가 출력보다 먼저 찍힌다. Ctrl-L 은 화면을 비워 한 장면에 명령 하나만 서게 한다
        'wiki': [
            (1.2, None),
            *typed('moai wiki search worker index -n 5', then=0.6),
            (4.0, CLEAR),
            *typed_later(lambda: f"moai wiki learn 'Test a paste, not only typing' --issue {demo_id('conflict')} -b - <<'LESSON'", then=0.4),
            *typed('## What happened', then=0.3),
            *typed('A pasted paragraph got a new id, so sync saw a delete plus an insert.', gap=0.02, then=0.3),
            *typed('## Do this instead', then=0.3),
            *typed('Paste a block in every editor test that touches ids.', gap=0.02, then=0.3),
            *typed('LESSON', then=0.6),
            (2.0, None),
            *typed('moai wiki ls', then=0.6),
            (4.0, CLEAR),
            *typed_later(lambda: f"moai show {demo_id('conflict')}", then=0.6),
            (5.0, None),
        ],
    }

def main():
    scene, repo = sys.argv[1], os.path.abspath(sys.argv[2])
    cols, rows = os.get_terminal_size(sys.stdout.fileno()) if sys.stdout.isatty() else (100, 30)
    # 녹화하는 사람의 환경이 화면에 새지 않게: 정체·키·색·말·설정은 데모의 것으로. HOME 과 moai 의 자리는
    # record-guide.sh 가 데모 전용 디렉터리(DEMO_HOME)에 지어 둔다 — init 이 적는 경로가 화면에 그대로 서기 때문이다
    home = os.environ['DEMO_HOME']
    bindir = os.path.join(home, '.local', 'bin')
    moai = os.path.join(bindir, 'moai')
    env = dict(TERM='xterm-256color', COLORTERM='truecolor', LANG='C.UTF-8', MOAI_LANG='en', MOAI_NO_UPDATE_CHECK='1',
               MOAI=moai, MOAI_CONFIG=os.path.join(home, '.config', 'moai', 'config.toml'),
               HOME=home, PATH=bindir + ':/usr/local/bin:/usr/bin:/bin',
               PS1='\\[\\e[38;5;245m\\]lantern\\[\\e[0m\\] \\[\\e[38;5;209m\\]$\\[\\e[0m\\] ')
    pid, fd = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        os.chdir(repo)
        os.execve('/bin/bash', ['bash', '--norc', '--noprofile', '-i'], env)
    out = sys.stdout.buffer
    steps = scenes(repo, env)[scene]
    # init 의 설정 화면은 그릴 자리를 정하려고 커서 위치를 묻는다(ESC[6n). 진짜 터미널이라면 답하겠지만
    # 여기는 이 스크립트가 터미널이라 직접 답한다. 셸에서 찍힌 줄바꿈 수로 줄을 셈하고, 화면 끝에서는 끝 줄에 선다
    seen = {'row': 1}

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
                if b'\x1b[6n' in data:
                    head = data.split(b'\x1b[6n', 1)[0]
                    row = min(rows, seen['row'] + head.count(b'\n'))
                    os.write(fd, f'\x1b[{row};1R'.encode())
                seen['row'] = min(rows, seen['row'] + data.count(b'\n'))
                out.write(data); out.flush()

    for wait, what in steps:
        pump(wait)
        if what is None:
            continue
        if callable(what):
            what = what()
        os.write(fd, what.encode())
    pump(0.3)
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass

if __name__ == '__main__':
    main()
