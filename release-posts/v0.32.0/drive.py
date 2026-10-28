#!/usr/bin/env python3
"""v0.32.0 소식의 TUI 장면. scripts/site-demo/drive.py 의 걸음 틀을 그대로 쓰고 장면만 더한다."""
import importlib.util, os, sys
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('sitedrive', os.path.join(here, '../../scripts/site-demo/drive.py'))
d = importlib.util.module_from_spec(spec); spec.loader.exec_module(d)
SPC, ENTER, ESC, TAB = d.SPC, d.ENTER, d.ESC, d.TAB
base = d.scenes
def scenes(repo):
    s = base(repo)
    key = lambda *seq, gap=0.6: [(gap, k) for k in seq]
    # 마감만 있는 줄(make.sh 가 시작을 지운다)에도 막대가 선다. 커서는 그 줄 위에 선다
    s['v32-gantt'] = [
        (2.0, SPC), (0.7, 'g'), (0.6, 'g'),
        (2.5, 't'), (1.5, '='),
        (2.0, 'j'), *key(*(['j'] * 9), gap=0.3),
        (3.0, None),
    ]
    # 목록에서 에픽을 펼쳐 이슈 하나를 고르고, 상세로 건너가 이력·작업 기록까지 내린다
    s['v32-detail'] = [
        (2.0, TAB), (1.4, 'j'), *key('j', gap=0.5),
        (1.5, '\x17'), (0.3, 'w'),
        *key(*(['j'] * 44), gap=0.2),
        (3.0, None),
    ]
    s['v32-board'] = [
        (1.8, SPC), (0.7, 'g'), (0.6, 'b'),
        (2.5, None),
    ]
    return s
d.scenes = scenes
d.main()
