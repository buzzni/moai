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
            if args[0] == 'display-message': return '0 0 10\n'
            return 'conversation\n› Ask Codex to do anything\n'
        with patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value='10 1 S 10 11 /bin/sh\n11 10 S 11 11 /usr/bin/codex\n'):
            pane.ready('%123')
            with patch.object(pane, 'tmux', side_effect=lambda *a: '1 0 10\n'):
                with self.assertRaises(ValueError): pane.ready('%123')
            with patch.object(pane, 'tmux', side_effect=lambda *a: '0 0 10\n' if a[0] == 'display-message' else 'Working\n› Ask Codex to do anything\n'):
                with self.assertRaises(ValueError): pane.ready('%123')
        for processes in ('10 1 S 10 10 /bin/sh\n', '10 1 S 10 10 /bin/sh\n11 10 S 11 10 /usr/bin/codex\n', '10 1 S 10 11 /bin/sh\n11 10 Z 11 11 /usr/bin/codex\n'):
            with patch.object(pane, 'tmux', side_effect=tmux), patch.object(pane.subprocess, 'check_output', return_value=processes):
                with self.assertRaises(ValueError): pane.ready('%123')

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

if __name__ == '__main__': unittest.main()
