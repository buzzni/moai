#!/usr/bin/env python3
"""Release-specific TUI scenes, using the shared demo's PTY driver."""

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
            # Click the first milestone's chevron; the second lane stays expanded.
            (0.7, "\x1b[<0;4;11M"), (0.1, "\x1b[<0;4;11m"),
            (3.0, None),
        ],
        "release-archive": [
            (2.0, drive.SPC), (0.7, "o"), (0.7, "o"),
            (3.0, None),
        ],
    })
    return all_scenes


drive.scenes = scenes
drive.main()
