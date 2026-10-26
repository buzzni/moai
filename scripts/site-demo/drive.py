#!/usr/bin/env python3
"""moai tui 를 pty 에 띄우고 정해 둔 키를 시간에 맞춰 흘린다. asciinema 가 이것의 출력을 녹화한다.

    asciinema rec --headless --window-size 120x34 -c 'scripts/site-demo/drive.py tour <저장소>' tour.cast

장면은 아래 SCENES 에 있다. 한 걸음은 (기다릴 초, 보낼 것) 이고, 보낼 것이 ('sh', 명령) 이면 키 대신
그 저장소에서 moai 명령을 돌린다 — TUI 는 파일이 바뀌면 스스로 다시 읽으니, 뒤에서 옮긴 카드가 화면에서 움직인다.
"""
import fcntl, os, pty, select, shlex, shutil, signal, struct, subprocess, sys, termios, time

SPC, ENTER, ESC, TAB, BKSP = ' ', '\r', '\x1b', '\t', '\x7f'

def scenes(repo):
    ids = dict(line.split('=', 1) for line in open(os.path.join(repo, '.demo-ids')).read().split())
    moai = os.environ['MOAI']
    def sh(cmd):
        return ('sh', cmd.format(moai=shlex.quote(moai), **ids))
    SAM = shlex.quote('Sam Rivera (sam@example.com)')
    def typed(text, gap=0.09):
        return [(gap, ch) for ch in text]
    def key(*seq, gap=0.6):
        return [(gap, k) for k in seq]
    def drag(x0, y, x1, wait=1.0, gap=0.09):
        # 왼 단추로 (x0, y) 를 눌러 x1 까지 끌고 놓는다 — SGR 마우스, 칸은 1 부터
        step = 1 if x1 > x0 else -1
        moves = [(gap, f'\x1b[<32;{x};{y}M') for x in range(x0 + step, x1 + step, step)]
        return [(wait, f'\x1b[<0;{x0};{y}M'), *moves, (0.3, f'\x1b[<0;{x1};{y}m')]
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
        # 간트 (v0.24.0 의 Discord 소식, moai-r96m). 계획한 날은 record-tui.sh 가 plan.sh 로 달아 둔다.
        # 제목 위의 노란 띠는 검색이 아니라 집은 줄이 도는 빛줄기다(draw.rs 의 shimmer) — 기능이라 그대로 두고, 정지 그림에는
        # 한 걸음이 굳어 찍힌다(moai-vj8w.fuh)
        # 줄의 차례는 record-tui.sh 가 데모 설정에 적는 order 로 굳힌다 — 기본 차례는 같은 p 끼리 seed 마다 새로 뜨는 id 로 갈려 자리가 안
        # 선다. 커서와 마우스는 'Sync attachments larger than 10 MB' 를 겨눈다(여섯째 줄, 화면 18 줄). 칸은 120x40
        # 창(make.sh 의 SIZE — record-tui.sh 의 기본 120x34 가 아니다. 낮은 창은 ▼ 줄을 접어 줄이 한 칸 올라간다)에서
        # 맞힌 것이다 — 맞춘 화면에서 하루가 한 칸이고 67 이 그 막대의 가운데, 42 가 줄과 날짜 사이의 금.
        # 창 크기나 seed.sh·plan.sh 의 날을 바꾸면 여기와 make.sh 의 시각을 같이 본다
        'gantt': [
            (2.0, SPC), (0.7, 'g'), (0.6, 'g'),
            (2.5, '='), (2.0, 't'), (2.0, '-'),
            (2.0, 'H'), *key('H', 'H', 'H', gap=0.5), (1.0, 'L'), *key('L', gap=0.5),
            (1.4, 't'), (2.0, 'z'),
            (2.0, 'j'), *key('j', 'j', 'j', 'j', gap=0.4),
            (1.4, '<'), *key('<', gap=0.8),
            (1.6, '>'), *key('>', gap=0.8),
            # 막대 끌기는 그 줄을 다시 쓴다 — 읽음은 쓴 사람을 안 가리니 Mira 자신의 쓰기도 그 줄을 [NEW] 로 세운다
            # (moai-vj8w.fuh). 화면이 다시 읽은 뒤 r 로 커서 줄을 읽는다. 둘을 합친 3.0 초는 옛 기다림 그대로라 make.sh 의 시각이 안 밀린다
            *drag(67, 18, 64, wait=2.0, gap=0.25),
            (1.2, 'r'),
            *drag(42, 20, 58, wait=1.8),
            (3.5, None),
        ],
        # v0.27.0 의 소식(moai-0fai): 간트 머리의 달 이름이 화면의 말을 따른다. make.sh 가 DEMO_LANG=ko 로 찍는다.
        # 계획한 날은 gantt 처럼 plan.sh 가 단다. 주 걸음(-)으로 밀면 머리에 달 이름이 선다
        'gantt-months': [
            (2.0, SPC), (0.7, 'g'), (0.6, 'g'),
            (2.5, 't'), (2.0, '-'),
            (3.0, None),
        ],
        # v0.27.0 의 소식(moai-0fai): 위키 탭(SPC g w)의 / 가 moai wiki search 의 차례로 페이지를 세운다 —
        # 그 낱말을 든 절이 높은 페이지가 먼저, 글자만 든 페이지가 뒤에
        'wiki-search': [
            (2.0, SPC), (0.7, 'g'), (0.6, 'w'),
            (2.0, '/'), *typed('sync worker', gap=0.12),
            (3.0, None),
        ],
        # v0.28.0 의 소식(moai-8kvc): ? 가 키 목록 창을 연다 — 화면 크기에 따라 한~세 열. make.sh 가 창 크기를 바꿔
        # 두 번 찍는다. 열고 굴리고(j) 다시 ? 로 닫는다
        'keys': [
            (2.5, '?'),
            (3.0, 'j'), *key('j', 'j', 'j', 'j', 'j', 'j', 'j', gap=0.25),
            (2.0, '?'),
            (2.0, None),
        ],
    }

def main():
    scene, repo = sys.argv[1], os.path.abspath(sys.argv[2])
    cols, rows = os.get_terminal_size(sys.stdout.fileno()) if sys.stdout.isatty() else (120, 34)
    moai = os.environ.get('MOAI', 'moai')
    moai = os.path.abspath(moai) if os.sep in moai else (shutil.which(moai) or moai)
    os.environ['MOAI'] = moai   # 장면의 뒤쪽 명령(sh)도 같은 바이너리를 부른다
    # 녹화하는 사람의 환경이 화면에 새지 않게: 정체·키·색·말은 데모의 것으로
    env = dict(os.environ, TERM='xterm-256color', MOAI_NO_UPDATE_CHECK='1', COLORTERM='truecolor', MOAI_LANG=os.environ.get('DEMO_LANG', 'en'),
               MOAI=moai, MOAI_CONFIG=os.path.join(repo, '.demo-home', 'config.toml'))
    for k in ('MOAI_NOW', 'MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR'):
        env.pop(k, None)
    pid, fd = pty.fork()
    if pid == 0:
        # 크기는 자식이 exec 하기 전에 제 터미널에 건다 — 부모가 나중에 걸면 TUI 가 0×0 을 먼저 읽을 수 있다
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        os.chdir(repo)
        os.execve(moai, [moai, 'tui'], env)
    out = sys.stdout.buffer
    steps = scenes(repo)[scene]

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
