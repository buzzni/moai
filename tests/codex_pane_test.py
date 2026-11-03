import importlib.util
import pathlib
import unittest
import tempfile
from unittest.mock import patch

path = pathlib.Path(__file__).parents[1] / 'src/skill_scripts/codex-pane.py'
spec = importlib.util.spec_from_file_location('codex_pane', path)
pane = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pane)

class ReadyTests(unittest.TestCase):
    def test_requires_live_process_and_empty_composer(self):
        def tmux(*args):
            if args[0] == 'display-message': return '0\t0\t10\t/repo/.worktrees/x\n'
            return 'conversation\n› Ask Codex to do anything\n'
        with patch.object(pane, 'places', return_value=['/repo']), patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value='10 1 S 10 11 /bin/sh\n11 10 S 11 11 /usr/bin/codex\n'):
            pane.ready('%123')
            with patch.object(pane, 'tmux', side_effect=lambda *a: '1\t0\t10\t/repo\n'):
                with self.assertRaises(ValueError): pane.ready('%123')
            with patch.object(pane, 'tmux', side_effect=lambda *a: '0\t0\t10\t/repo\n' if a[0] == 'display-message' else 'Working\n› Ask Codex to do anything\n'):
                with self.assertRaises(ValueError): pane.ready('%123')
        for processes in ('10 1 S 10 10 /bin/sh\n', '10 1 S 10 10 /bin/sh\n11 10 S 11 10 /usr/bin/codex\n', '10 1 S 10 11 /bin/sh\n11 10 Z 11 11 /usr/bin/codex\n'):
            with patch.object(pane, 'places', return_value=['/repo']), patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value=processes):
                with self.assertRaises(ValueError): pane.ready('%123')

    def test_refuses_a_pane_outside_this_repository(self):
        procs = '10 1 S 10 11 /bin/sh\n11 10 S 11 11 /usr/bin/codex\n'
        for path in ('/elsewhere', '/repository-next-door', ''):
            def tmux(*args, path=path):
                return f'0\t0\t10\t{path}\n' if args[0] == 'display-message' else '› Ask Codex to do anything\n'
            with patch.object(pane, 'places', return_value=['/repo', '/repository']), \
                 patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value=procs):
                with self.assertRaises(ValueError, msg=path) as caught: pane.ready('%123')
                self.assertIn('outside this repository', str(caught.exception))
        with patch.object(pane.subprocess, 'check_output', side_effect=pane.subprocess.CalledProcessError(128, 'git')):
            with self.assertRaises(ValueError): pane.places()

    def test_send_rechecks_and_clear_checks_before_enter(self):
        calls = []
        with tempfile.TemporaryDirectory() as tmp:
            file = pathlib.Path(tmp) / 'assignment'
            file.write_text('Read this whole assignment\nsecond line')
            with patch.object(pane.sys, 'argv', ['codex-pane.py', 'send', '%123', str(file)]), \
                 patch.object(pane, 'ready', side_effect=lambda *a, **k: calls.append(('ready', a, k))), \
                 patch.object(pane, 'tmux', side_effect=lambda *a: calls.append(('tmux', a)) or ''), \
                 patch.object(pane.time, 'sleep'):
                pane.main()
        operations = [entry[1][0] if entry[0] == 'tmux' else 'ready' for entry in calls]
        self.assertEqual(operations, ['ready', 'load-buffer', 'ready', 'paste-buffer', 'ready', 'send-keys'])
        calls.clear()
        with patch.object(pane.sys, 'argv', ['codex-pane.py', 'clear', '%123']), \
             patch.object(pane, 'ready', side_effect=lambda *a, **k: calls.append(('ready', a, k)) or str(len(calls))), \
             patch.object(pane, 'tmux', side_effect=lambda *a: calls.append(('tmux', a)) or ''), \
             patch.object(pane.time, 'sleep'):
            pane.main()
        operations = [entry[1][0] if entry[0] == 'tmux' else 'ready' for entry in calls]
        self.assertEqual(operations, ['ready', 'send-keys', 'ready', 'send-keys', 'ready'])

    def test_pasted_words_do_not_refuse_their_own_paste(self):
        procs = '10 1 S 10 11 /bin/sh\n11 10 S 11 11 /usr/bin/codex\n'
        screen = 'conversation\n› Read this assignment: select the approval flow\n  Work running alongside: confirm\n'
        def tmux(*args):
            return '0\t0\t10\t/repo\n' if args[0] == 'display-message' else screen
        with patch.object(pane, 'places', return_value=['/repo']), patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value=procs):
            pane.ready('%123', 'Read this as', prefix=True)
            with self.assertRaises(ValueError): pane.ready('%123')

    def test_a_long_paste_folded_by_codex_is_still_ours(self):
        procs = '10 1 S 10 11 /bin/sh\n11 10 S 11 11 /usr/bin/codex\n'
        screen = 'conversation\n› [Pasted Content 1234 chars]\n'
        def tmux(*args):
            return '0\t0\t10\t/repo\n' if args[0] == 'display-message' else screen
        with patch.object(pane, 'places', return_value=['/repo']), patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value=procs):
            pane.ready('%123', ('You are hand', '[Pasted Content '), prefix=True)
            with self.assertRaises(ValueError): pane.ready('%123', 'You are hand', prefix=True)

    def test_a_refused_paste_takes_its_buffer_back(self):
        calls = []
        def ready(*a, **k):
            calls.append(('ready',))
            if len([c for c in calls if c == ('ready',)]) == 2: raise ValueError('composer is not the expected input')
        with tempfile.TemporaryDirectory() as tmp:
            file = pathlib.Path(tmp) / 'assignment'
            file.write_text('Read this whole assignment\n')
            with patch.object(pane.sys, 'argv', ['codex-pane.py', 'send', '%123', str(file)]), \
                 patch.object(pane, 'ready', side_effect=ready), \
                 patch.object(pane, 'tmux', side_effect=lambda *a: calls.append(('tmux', a[0])) or ''):
                with self.assertRaises(ValueError): pane.main()
        self.assertEqual(calls, [('ready',), ('tmux', 'load-buffer'), ('ready',), ('tmux', 'delete-buffer')])

    def test_settle_looks_again_until_the_screen_agrees(self):
        answers = [ValueError('composer is not the expected input'), 'drawn']
        def ready(*a, **k):
            answer = answers.pop(0)
            if isinstance(answer, Exception): raise answer
            return answer
        with patch.object(pane, 'ready', side_effect=ready), patch.object(pane.time, 'sleep'):
            self.assertEqual(pane.settle('%123', 'Read', prefix=True), 'drawn')
        with patch.object(pane, 'ready', return_value='same'), patch.object(pane.time, 'sleep'), \
             patch.object(pane.time, 'monotonic', side_effect=[0, 0, 10]):
            with self.assertRaises(ValueError): pane.settle('%123', differs='same')

if __name__ == '__main__': unittest.main()
