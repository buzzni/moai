#!/usr/bin/env python3
"""v0.38.0 TUI board, list and archive scenes, using the shared PTY driver."""

import importlib.util
import os
import sys


root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
spec = importlib.util.spec_from_file_location("site_demo_drive", os.path.join(root, "scripts/site-demo/drive.py"))
drive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drive)
original_scenes = drive.scenes


def scenes(repo):
    all_scenes = original_scenes(repo)
    all_scenes.update({
        "release-board": [
            (1.8, drive.SPC), (0.7, "g"), (0.6, "b"),
            (1.5, drive.SPC), (0.7, "v"), (0.6, "d"), (0.5, drive.ESC),
            # Pick an in-progress card to show its reversed black status band.
            (0.7, "\x1b[<0;60;14M"), (0.1, "\x1b[<0;60;14m"),
            (3.0, None),
        ],
        "release-list": [(3.5, None)],
        "release-archive": [
            (2.0, drive.SPC), (0.7, "o"), (0.7, "o"),
            (3.0, None),
        ],
    })
    return all_scenes


drive.scenes = scenes
drive.main()
